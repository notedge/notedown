//! PDF import via lightweight object and content-stream parsing.

mod content;
mod encoding;
mod fonts;
mod images;
mod metadata;
mod objects;
mod streams;
mod syntax;
mod types;

#[doc(hidden)]
pub mod test_support;

use notedown_ir::{Block, DocumentGraph, DocumentId, Inline, LossMarker, SemanticStatus};

use crate::FormatError;

use self::{
    content::extract_text_operators,
    fonts::page_font_maps,
    images::page_images,
    metadata::{append_image_asset, pdf_metadata, report_unsupported_pdf_structures, split_pdf_paragraphs},
    objects::page_content_ids,
    streams::stream_contents,
    types::ContentEvent,
};

/// Import basic page text from a PDF into semantic paragraphs.
pub fn import_pdf_bytes(label: &str, bytes: &[u8]) -> Result<DocumentGraph, FormatError> {
    if !bytes.starts_with(b"%PDF-") {
        return Err(FormatError::invalid_input("input is not a PDF file"));
    }
    let mut graph = DocumentGraph::new(DocumentId(1));
    if let Some(metadata) = pdf_metadata(bytes) {
        graph.metadata = metadata;
    }
    report_unsupported_pdf_structures(bytes, &mut graph);
    let page_content_ids = page_content_ids(bytes);
    let streams = stream_contents(bytes);
    let font_maps = page_font_maps(bytes, &streams)?;
    let images = page_images(bytes, &streams)?;
    let use_page_filter = !page_content_ids.is_empty();
    let selected = if use_page_filter {
        page_content_ids.iter().filter_map(|id| streams.iter().find(|stream| stream.object_id == Some(*id))).collect::<Vec<_>>()
    }
    else {
        streams.iter().collect::<Vec<_>>()
    };
    for stream in selected {
        let decoded = streams::decode_stream(stream.dictionary, stream.data)?;
        let extracted = extract_text_operators(&decoded, &font_maps);
        if extracted.spacing_adjustments {
            graph.push_loss(LossMarker {
                code: "import.pdf.text_spacing".into(),
                message: "TJ numeric spacing adjustments are ignored while preserving the adjacent text strings".into(),
                status: SemanticStatus::Partial,
            });
        }
        for event in extracted.events {
            match event {
                ContentEvent::Text(text) => {
                    for paragraph in split_pdf_paragraphs(&text) {
                        graph.push_block(Block::Paragraph { content: vec![Inline::Text { text: paragraph.to_string() }] });
                    }
                }
                ContentEvent::NamedImage(image_name) => {
                    if let Some(image) = images.get(&image_name) {
                        append_image_asset(&mut graph, image);
                    }
                    else {
                        graph.push_loss(LossMarker {
                            code: "import.pdf.unresolved_xobject".into(),
                            message: format!("content stream references named XObject `/{image_name}` that is not a supported image"),
                            status: SemanticStatus::Unresolved,
                        });
                    }
                }
                ContentEvent::InlineImage(image) => append_image_asset(&mut graph, &image),
            }
        }
    }
    if graph.blocks.is_empty() {
        graph.push_loss(LossMarker {
            code: "import.pdf.no_extractable_text".into(),
            message: format!("{label}: no supported text operators were found"),
            status: SemanticStatus::Partial,
        });
    }
    graph.push_loss(LossMarker { code: "import.pdf.partial_coverage".into(), message: "PDF import preserves basic page text and JPEG or 8-bit grayscale/RGB Flate image assets in content-stream order, but not general layout, fonts, indexed or alpha images, annotations, forms, or encryption".into(), status: SemanticStatus::Partial });
    Ok(graph)
}

/// Import a PDF file from disk.
pub fn import_pdf(path: impl AsRef<std::path::Path>) -> Result<DocumentGraph, FormatError> {
    let path = path.as_ref();
    let bytes = std::fs::read(path).map_err(|error| FormatError::invalid_input(format!("failed to read {}: {error}", path.display())))?;
    import_pdf_bytes(&path.display().to_string(), &bytes)
}
