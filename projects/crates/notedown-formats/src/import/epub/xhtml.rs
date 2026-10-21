use notedown_ir::{Block, DocumentGraph, Inline, LossMarker, SemanticStatus};

use super::{
    assets::{image_inline, register_image_from_src},
    ast_lowering::{blocks_from_html_document, register_spine_images_from_document},
    html_extensions::parse_html_bytes,
};
use crate::FormatError;

/// Lower XHTML spine content into `notedown-ir` blocks via `oak-html`.
pub fn blocks_from_xhtml(xml: &[u8]) -> Result<Vec<Block>, FormatError> {
    let mut scratch = DocumentGraph::new(notedown_ir::DocumentId(0));
    blocks_from_xhtml_with_context(xml, &mut scratch, None)
}

/// Lower XHTML spine content and optionally register embedded image assets.
pub fn blocks_from_xhtml_with_context(xml: &[u8], graph: &mut DocumentGraph, member_path: Option<&str>) -> Result<Vec<Block>, FormatError> {
    let text = prepare_xhtml_for_oak_html(&String::from_utf8_lossy(xml));
    let document = parse_html_bytes(text.as_bytes())?;
    register_spine_images_from_document(&document, graph, member_path)?;
    let (blocks, losses) = blocks_from_html_document(&document, graph, member_path);

    if !losses.is_empty() && blocks.is_empty() {
        return Err(FormatError::parse(
            "epub",
            losses.first().map(|loss| loss.message.clone()).unwrap_or_else(|| "xhtml produced no blocks".into()),
        ));
    }

    Ok(blocks)
}

/// Loss markers discovered while lowering XHTML (CSS/SVG still pending).
pub fn losses_from_xhtml(xml: &[u8]) -> Result<Vec<LossMarker>, FormatError> {
    let text = prepare_xhtml_for_oak_html(&String::from_utf8_lossy(xml));
    let document = parse_html_bytes(text.as_bytes())?;
    let (_, losses) = blocks_from_html_document(&document, &mut DocumentGraph::new(notedown_ir::DocumentId(0)), None);
    let mut losses = losses;
    losses.extend(scan_markup_losses(text.as_ref()));
    Ok(losses)
}

fn scan_markup_losses(text: &str) -> Vec<LossMarker> {
    let mut losses = Vec::new();
    let lower = text.to_ascii_lowercase();
    if lower.contains(" style=\"") || lower.contains(" style='") {
        losses.push(LossMarker {
            code: "import.epub.inline_style_unsupported".into(),
            message: "inline style attributes are not lowered into Notedown IR yet".into(),
            status: SemanticStatus::Unsupported,
        });
    }
    if lower.contains("<style") {
        losses.push(LossMarker {
            code: "import.epub.css_inline_block_unsupported".into(),
            message: "embedded <style> blocks are not lowered into Notedown IR yet".into(),
            status: SemanticStatus::Unsupported,
        });
    }
    losses
}

pub(crate) fn quote_block_from_blocks(blocks: Vec<Block>) -> Block {
    let mut content = Vec::new();
    for (index, block) in blocks.iter().enumerate() {
        if index > 0 {
            push_inline(&mut content, Inline::Text { text: "\n".into() });
        }
        match block {
            Block::Paragraph { content: paragraph } => {
                for inline in paragraph {
                    push_inline(&mut content, inline.clone());
                }
            }
            Block::Quote { content: quote } => {
                for inline in quote {
                    push_inline(&mut content, inline.clone());
                }
            }
            Block::Section { title, .. } => {
                for inline in title {
                    push_inline(&mut content, inline.clone());
                }
            }
            Block::Code { content: code, .. } => {
                push_inline(&mut content, Inline::Text { text: code.clone() });
            }
            _ => {}
        }
    }
    if content.is_empty() { Block::Quote { content: vec![Inline::Text { text: String::new() }] } } else { Block::Quote { content } }
}

pub(crate) fn push_inline(inlines: &mut Vec<Inline>, inline: Inline) {
    if let Inline::Text { text: right } = inline {
        if let Some(Inline::Text { text: left }) = inlines.last_mut() {
            left.push_str(&right);
            return;
        }
        if right.is_empty() {
            return;
        }
        inlines.push(Inline::Text { text: right });
        return;
    }
    inlines.push(inline);
}

pub(crate) fn normalize_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Normalize EPUB XHTML into HTML that `oak-html` can parse reliably today.
fn prepare_xhtml_for_oak_html(xml: &str) -> String {
    let mut text = xml.trim().to_string();
    if text.starts_with("<?") {
        if let Some(end) = text.find("?>") {
            text = text[end + 2..].trim_start().to_string();
        }
    }
    text = text.replace("xmlns=\"http://www.w3.org/1999/xhtml\"", "");
    text = text.replace("xmlns='http://www.w3.org/1999/xhtml'", "");
    text = text.replace("xml:lang=", "lang=");
    text
}

pub(crate) fn register_and_image_inline(src: &str, alt: &str, graph: &mut DocumentGraph, member_path: Option<&str>) -> Option<Inline> {
    let asset = member_path.and_then(|member_path| register_image_from_src(graph, member_path, src));
    image_inline(src, alt, asset)
}

pub(crate) fn language_from_class(class: &str) -> String {
    for token in class.split_whitespace() {
        if let Some(language) = token.strip_prefix("language-") {
            if !language.is_empty() {
                return language.to_string();
            }
        }
    }
    class.split_whitespace().next().filter(|token| !token.is_empty()).unwrap_or(class).to_string()
}
