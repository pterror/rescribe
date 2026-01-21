//! EPUB reader for rescribe.
//!
//! Parses EPUB files into rescribe's document IR by extracting and parsing
//! the XHTML content from each chapter.
//!
//! # Example
//!
//! ```ignore
//! use rescribe_read_epub::parse_file;
//!
//! let result = parse_file("book.epub")?;
//! let doc = result.value;
//! ```

use epub::doc::EpubDoc;
use rescribe_core::{
    ConversionResult, Document, FidelityWarning, Node, ParseError, Properties, Resource,
    ResourceId, ResourceMap, Severity, SourceInfo, WarningKind,
};
use rescribe_std::{node, prop};
use std::io::{Read, Seek};
use std::path::Path;

/// Parse an EPUB file from a path.
pub fn parse_file<P: AsRef<Path>>(path: P) -> Result<ConversionResult<Document>, ParseError> {
    let doc = EpubDoc::new(path)
        .map_err(|e| ParseError::Invalid(format!("Failed to open EPUB: {}", e)))?;
    convert_epub(doc)
}

/// Parse EPUB from a reader that implements Read + Seek.
pub fn parse<R: Read + Seek>(reader: R) -> Result<ConversionResult<Document>, ParseError> {
    let doc = EpubDoc::from_reader(reader)
        .map_err(|e| ParseError::Invalid(format!("Failed to parse EPUB: {}", e)))?;
    convert_epub(doc)
}

/// Parse EPUB from bytes.
pub fn parse_bytes(bytes: &[u8]) -> Result<ConversionResult<Document>, ParseError> {
    let cursor = std::io::Cursor::new(bytes.to_vec());
    parse(cursor)
}

fn convert_epub<R: Read + Seek>(
    mut doc: EpubDoc<R>,
) -> Result<ConversionResult<Document>, ParseError> {
    let mut converter = Converter::new();

    // Extract metadata
    let metadata = extract_metadata(&doc);

    // Convert all chapters
    let children = converter.convert_chapters(&mut doc)?;

    let document = Document {
        content: Node::new(node::DOCUMENT).children(children),
        resources: converter.resources,
        metadata,
        source: Some(SourceInfo {
            format: "epub".to_string(),
            metadata: Properties::new(),
        }),
    };

    Ok(ConversionResult::with_warnings(
        document,
        converter.warnings,
    ))
}

struct Converter {
    warnings: Vec<FidelityWarning>,
    resources: ResourceMap,
}

impl Converter {
    fn new() -> Self {
        Self {
            warnings: Vec::new(),
            resources: ResourceMap::new(),
        }
    }

    fn warn(&mut self, message: impl Into<String>) {
        self.warnings.push(FidelityWarning::new(
            Severity::Minor,
            WarningKind::FeatureLost("epub".to_string()),
            message,
        ));
    }

    fn add_resource(&mut self, data: Vec<u8>, content_type: &str) -> ResourceId {
        let id = ResourceId::new();
        let resource = Resource::new(content_type.to_string(), data);
        self.resources.insert(id.clone(), resource);
        id
    }

    fn convert_chapters<R: Read + Seek>(
        &mut self,
        doc: &mut EpubDoc<R>,
    ) -> Result<Vec<Node>, ParseError> {
        let mut all_children = Vec::new();

        // Get number of chapters
        let num_chapters = doc.get_num_chapters();

        for i in 0..num_chapters {
            // Set current chapter
            if !doc.set_current_chapter(i) {
                continue;
            }

            // Get chapter content
            let content = match doc.get_current_str() {
                Some((content, _mime)) => content,
                None => continue,
            };

            // Get chapter title if available (from TOC)
            let chapter_title = doc.get_current_id();

            // Parse XHTML content using the HTML reader
            let html_result = rescribe_read_html::parse(&content);

            match html_result {
                Ok(result) => {
                    // Add warnings from HTML parsing
                    self.warnings.extend(result.warnings);

                    // Add chapter heading if we have a title and multiple chapters
                    if num_chapters > 1 {
                        // Only add heading if chapter title looks meaningful
                        if let Some(ref title) = chapter_title
                            && !title.is_empty()
                            && !title.starts_with("item")
                        {
                            let heading = Node::new(node::HEADING)
                                .prop(prop::LEVEL, 1i64)
                                .child(Node::new(node::TEXT).prop(prop::CONTENT, title.clone()));
                            all_children.push(heading);
                        }
                    }

                    // Add the chapter's content (children of the document node)
                    all_children.extend(result.value.content.children);
                }
                Err(e) => {
                    self.warn(format!("Failed to parse chapter {}: {}", i, e));
                }
            }
        }

        // Try to extract cover image
        self.extract_images(doc)?;

        Ok(all_children)
    }

    fn extract_images<R: Read + Seek>(&mut self, doc: &mut EpubDoc<R>) -> Result<(), ParseError> {
        // Try to get cover image
        if let Some((cover_data, mime_type)) = doc.get_cover() {
            let _resource_id = self.add_resource(cover_data, &mime_type);
            // Note: We don't automatically insert the cover; it should be in the content
        }

        Ok(())
    }
}

fn extract_metadata<R: Read + Seek>(doc: &EpubDoc<R>) -> Properties {
    let mut metadata = Properties::new();

    if let Some(title) = doc.mdata("title") {
        metadata.set("title", title.value.clone());
    }
    if let Some(creator) = doc.mdata("creator") {
        metadata.set("author", creator.value.clone());
    }
    if let Some(language) = doc.mdata("language") {
        metadata.set("language", language.value.clone());
    }
    if let Some(publisher) = doc.mdata("publisher") {
        metadata.set("publisher", publisher.value.clone());
    }
    if let Some(description) = doc.mdata("description") {
        metadata.set("description", description.value.clone());
    }
    if let Some(date) = doc.mdata("date") {
        metadata.set("date", date.value.clone());
    }
    if let Some(identifier) = doc.mdata("identifier") {
        metadata.set("identifier", identifier.value.clone());
    }

    metadata
}

#[cfg(test)]
mod tests {
    // Tests require actual EPUB files
    // Integration tests can be added with test fixtures
}
