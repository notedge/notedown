use notedown_ir::{Block, DocumentGraph, Inline, LossMarker, SemanticStatus};

use super::ast_lowering::{blocks_from_html_document, register_spine_images_from_document};
use super::assets::{image_inline, register_image_from_src};
use super::oak_html_util::parse_html_bytes;
use crate::FormatError;

/// Lower XHTML spine content into `notedown-ir` blocks via `oak-html`.
pub fn blocks_from_xhtml(xml: &[u8]) -> Result<Vec<Block>, FormatError> {
    let mut scratch = DocumentGraph::new(notedown_ir::DocumentId(0));
    blocks_from_xhtml_with_context(xml, &mut scratch, None)
}

/// Lower XHTML spine content and optionally register embedded image assets.
pub fn blocks_from_xhtml_with_context(
    xml: &[u8],
    graph: &mut DocumentGraph,
    member_path: Option<&str>,
) -> Result<Vec<Block>, FormatError> {
    let text = prepare_xhtml_for_oak_html(&String::from_utf8_lossy(xml));
    let document = parse_html_bytes(text.as_bytes())?;
    register_spine_images_from_document(&document, graph, member_path)?;
    let (blocks, losses) = blocks_from_html_document(&document, graph, member_path);

    if !losses.is_empty() && blocks.is_empty() {
        return Err(FormatError::parse(
            "epub",
            losses
                .first()
                .map(|loss| loss.message.clone())
                .unwrap_or_else(|| "xhtml produced no blocks".into()),
        ));
    }

    Ok(blocks)
}

/// Loss markers discovered while lowering XHTML (CSS/SVG still pending).
pub fn losses_from_xhtml(xml: &[u8]) -> Result<Vec<LossMarker>, FormatError> {
    let text = prepare_xhtml_for_oak_html(&String::from_utf8_lossy(xml));
    let document = parse_html_bytes(text.as_bytes())?;
    let (_, losses) = blocks_from_html_document(
        &document,
        &mut DocumentGraph::new(notedown_ir::DocumentId(0)),
        None,
    );
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
    if content.is_empty() {
        Block::Quote {
            content: vec![Inline::Text { text: String::new() }],
        }
    } else {
        Block::Quote { content }
    }
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

pub(crate) fn register_and_image_inline(
    src: &str,
    alt: &str,
    graph: &mut DocumentGraph,
    member_path: Option<&str>,
) -> Option<Inline> {
    if let Some(member_path) = member_path {
        register_image_from_src(graph, member_path, src);
    }
    image_inline(src, alt)
}

pub(crate) fn language_from_class(class: &str) -> String {
    for token in class.split_whitespace() {
        if let Some(language) = token.strip_prefix("language-") {
            if !language.is_empty() {
                return language.to_string();
            }
        }
    }
    class
        .split_whitespace()
        .next()
        .filter(|token| !token.is_empty())
        .unwrap_or(class)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_XHTML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<html xmlns="http://www.w3.org/1999/xhtml">
  <body>
    <h1>Chapter One</h1>
    <p>Hello <strong>EPUB</strong></p>
  </body>
</html>"#;

    #[test]
    fn oak_html_lowers_headings_paragraphs_and_inline_styles() {
        let blocks = blocks_from_xhtml(SAMPLE_XHTML.as_bytes()).expect("lower xhtml");
        assert!(blocks.len() >= 2, "blocks={blocks:?}");
        let markdown = notedown_ir::DocumentGraph::new(notedown_ir::DocumentId(1));
        let mut graph = markdown;
        for block in blocks {
            graph.push_block(block);
        }
        let exported = crate::export::markdown::export_markdown(&graph).expect("export");
        assert!(exported.contains("# Chapter One"));
        assert!(exported.contains("**EPUB**"));
    }

    #[test]
    fn oak_html_lowers_tables_and_nested_lists() {
        const XHTML: &str = r#"<html><body>
<table><tr><th>H</th><td>V</td></tr></table>
<ul><li>Top<ul><li>Nested</li></ul></li></ul>
</body></html>"#;
        let mut graph = notedown_ir::DocumentGraph::new(notedown_ir::DocumentId(2));
        let blocks = blocks_from_xhtml_with_context(XHTML.as_bytes(), &mut graph, None)
            .expect("lower xhtml");
        assert!(
            blocks.iter().any(|block| matches!(block, Block::Table { .. })),
            "blocks={blocks:?}"
        );
        for block in blocks {
            graph.push_block(block);
        }
        let exported = crate::export::markdown::export_markdown(&graph).expect("export");
        assert!(exported.contains("| H | V |"));
        assert!(exported.contains("- Top"));
        assert!(exported.contains("  - Nested"));
    }

    #[test]
    fn css_selector_registers_spine_images() {
        const XHTML: &str = r#"<html><body>
<p><img src="images/cover.png" alt="Cover"/></p>
<svg><image xlink:href="images/diagram.svg"/></svg>
</body></html>"#;
        let mut graph = notedown_ir::DocumentGraph::new(notedown_ir::DocumentId(3));
        let blocks = blocks_from_xhtml_with_context(
            XHTML.as_bytes(),
            &mut graph,
            Some("OEBPS/chapter.xhtml"),
        )
        .expect("lower xhtml");
        assert!(!blocks.is_empty());
        assert_eq!(graph.assets.len(), 2);
        assert!(
            graph
                .assets
                .iter()
                .any(|asset| asset.source.as_deref() == Some("OEBPS/images/cover.png"))
        );
        assert!(
            graph
                .assets
                .iter()
                .any(|asset| asset.source.as_deref() == Some("OEBPS/images/diagram.svg"))
        );
    }

    #[test]
    fn ast_lowering_handles_blockquote_pre_and_hr_without_body_wrapper() {
        const XHTML: &str = r#"<blockquote><p>Quoted line</p></blockquote>
<pre><code class="language-rust">fn main() {}</code></pre><hr/>"#;
        let blocks = blocks_from_xhtml(XHTML.as_bytes()).expect("lower xhtml");
        assert!(
            blocks.iter().any(|block| matches!(block, Block::Quote { .. })),
            "blocks={blocks:?}"
        );
        assert!(
            blocks.iter().any(|block| matches!(
                block,
                Block::Code { content, .. } if content.contains("fn main")
            )),
            "blocks={blocks:?}"
        );
        assert!(
            blocks.iter().any(|block| matches!(
                block,
                Block::Opaque { kind, .. } if kind == "thematic_break"
            )),
            "blocks={blocks:?}"
        );
    }

    #[test]
    fn oak_html_lowers_container_blockquote_code_and_hr() {
        const XHTML: &str = r#"<html><body>
<section><div><p>Wrapped</p></div></section>
<blockquote><p>Quote</p></blockquote>
<pre><code class="language-js">console.log(1)</code></pre>
<hr/>
</body></html>"#;
        let mut graph = notedown_ir::DocumentGraph::new(notedown_ir::DocumentId(3));
        let blocks = blocks_from_xhtml_with_context(XHTML.as_bytes(), &mut graph, None)
            .expect("lower xhtml");
        for block in blocks {
            graph.push_block(block);
        }
        let exported = crate::export::markdown::export_markdown(&graph).expect("export");
        assert!(exported.contains("Wrapped"));
        assert!(exported.contains("> Quote"));
        assert!(exported.contains("```js"));
        assert!(exported.contains("console.log(1)"));
        assert!(exported.contains("---"));
    }
}
