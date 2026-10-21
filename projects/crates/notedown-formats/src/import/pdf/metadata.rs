use super::{
    encoding::{decode_pdf_string, decode_text_bytes},
    objects::reference_ids,
    syntax::{hex_bytes, literal_close, skip_space},
    types::PdfImage,
};

use notedown_ir::{Asset, AssetId, AssetKind, Block, DocumentGraph, DocumentMetadata, Inline, LossMarker, SemanticStatus};

pub(crate) fn split_pdf_paragraphs(text: &str) -> impl Iterator<Item = &str> {
    text.split('\n').filter(|paragraph| !paragraph.is_empty())
}
pub(crate) fn report_unsupported_pdf_structures(bytes: &[u8], graph: &mut DocumentGraph) {
    if bytes.windows(7).any(|window| window == b"/Annots") {
        graph.push_loss(LossMarker {
            code: "import.pdf.annotations".into(),
            message: "PDF annotations are detected but annotation ranges, URI actions, and widget content are not represented in notedown-ir"
                .into(),
            status: SemanticStatus::Partial,
        });
    }
    if bytes.windows(9).any(|window| window == b"/AcroForm") {
        graph.push_loss(LossMarker {
            code: "import.pdf.forms".into(),
            message:
                "PDF AcroForm fields are detected but form names, values, appearances, and widget geometry are not represented in notedown-ir"
                    .into(),
            status: SemanticStatus::Partial,
        });
    }
    if bytes.windows(8).any(|window| window == b"/Encrypt") {
        graph.push_loss(LossMarker {
            code: "import.pdf.encryption".into(),
            message: "PDF encryption is detected and encrypted object semantics are not imported".into(),
            status: SemanticStatus::Unsupported,
        });
    }
}

pub(crate) fn pdf_metadata(bytes: &[u8]) -> Option<DocumentMetadata> {
    let info_position = bytes.windows(5).position(|window| window == b"/Info")?;
    let info_ref = reference_ids(bytes.get(info_position + 5..)?).first().copied()?;
    let marker = format!("{info_ref} 0 obj");
    let start = bytes.windows(marker.len()).position(|window| window == marker.as_bytes())? + marker.len();
    let end = bytes[start..].windows(6).position(|window| window == b"endobj")? + start;
    let object = &bytes[start..end];
    let title = pdf_dictionary_string(object, b"/Title");
    let author = pdf_dictionary_string(object, b"/Author");
    let keywords = pdf_dictionary_string(object, b"/Keywords");
    if title.is_none() && author.is_none() && keywords.is_none() {
        return None;
    }
    Some(DocumentMetadata {
        title,
        language: None,
        authors: author.into_iter().collect(),
        tags: keywords
            .map(|value| value.split(',').map(str::trim).filter(|item| !item.is_empty()).map(str::to_owned).collect())
            .unwrap_or_default(),
    })
}

pub(crate) fn pdf_dictionary_string(object: &[u8], key: &[u8]) -> Option<String> {
    let start = object.windows(key.len()).position(|window| window == key)? + key.len();
    let open = skip_space(object, start);
    match object.get(open)? {
        b'(' => {
            let close = literal_close(object, open + 1)?;
            Some(decode_pdf_string(&object[open + 1..close]))
        }
        b'<' if object.get(open + 1) != Some(&b'<') => {
            let close = object[open + 1..].iter().position(|byte| *byte == b'>')? + open + 1;
            let value = &object[open + 1..close];
            if value.iter().any(|byte| !byte.is_ascii_whitespace() && !byte.is_ascii_hexdigit()) {
                return None;
            }
            Some(decode_text_bytes(&hex_bytes(value)))
        }
        _ => None,
    }
}
pub(crate) fn append_image_asset(graph: &mut DocumentGraph, image: &PdfImage) {
    let asset_id =
        match graph.assets.iter().find(|asset| asset.kind == AssetKind::Image && asset.source.as_deref() == Some(image.source.as_str())) {
            Some(asset) => asset.id,
            None => {
                let asset_id = AssetId(graph.assets.len() as u64 + 1);
                graph.push_asset(Asset {
                    id: asset_id,
                    kind: AssetKind::Image,
                    content_identity: None,
                    source: Some(image.source.clone()),
                    media_type: Some(image.media_type.to_owned()),
                    status: SemanticStatus::Resolved,
                    bytes: Some(image.bytes.clone()),
                });
                asset_id
            }
        };
    graph.push_block(Block::Paragraph {
        content: vec![Inline::Image { source: image.source.clone(), alt: Vec::new(), title: None, asset: Some(asset_id) }],
    });
}
