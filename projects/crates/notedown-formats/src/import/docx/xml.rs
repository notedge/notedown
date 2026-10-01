use std::collections::{HashMap, HashSet};

use notedown_ir::{
    Asset, AssetId, AssetKind, Block, DocumentGraph, DocumentId, Inline, ListItem, LossMarker,
    SemanticStatus,
};
use oak_xml::ast::{XmlElement, XmlValue};

use super::footnotes::FootnoteCatalog;
use super::numbering::NumberingCatalog;
use super::oak_xml_util::{
    attribute_value, bool_from_element, child_element, document_root, element_is, element_text,
    elements_by_local_name, parse_xml_bytes, u32_attribute,
};
use super::table::{append_cell_paragraph, push_table_block, TableState};
use crate::FormatError;

/// Parses `word/document.xml` body content into a flat `DocumentGraph`.
pub fn parse_document_xml(
    xml: &[u8],
    rels: &HashMap<String, String>,
    numbering: &NumberingCatalog,
    footnotes: &FootnoteCatalog,
    graph: &mut DocumentGraph,
) -> Result<HashSet<u32>, FormatError> {
    let value = parse_xml_bytes(xml)?;
    let root = document_root(&value)?;
    let body = child_element(root, "body").ok_or_else(|| {
        FormatError::parse("docx", "word/document.xml is missing w:body")
    })?;

    let mut referenced_footnotes = HashSet::new();
    let mut body_state = BodyState::default();

    for child in body.children.iter().filter_map(XmlValue::as_element) {
        if element_is(child, "tbl") {
            flush_pending_list(graph, &mut body_state, numbering);
            let table = parse_table_element(
                child,
                rels,
                graph,
                footnotes,
                &mut referenced_footnotes,
            );
            push_table_block(graph, table);
        } else if element_is(child, "p") {
            let paragraph = parse_paragraph_element(
                child,
                rels,
                graph,
                footnotes,
                &mut referenced_footnotes,
            );
            finish_paragraph(graph, &mut body_state, &paragraph, numbering);
        }
    }

    flush_pending_list(graph, &mut body_state, numbering);

    if graph.blocks.is_empty() {
        graph.push_loss(LossMarker {
            code: "reader.docx.empty_body".into(),
            message: "word/document.xml contained no paragraphs".into(),
            status: SemanticStatus::Partial,
        });
    }

    Ok(referenced_footnotes)
}

fn parse_table_element(
    table_element: &XmlElement,
    rels: &HashMap<String, String>,
    graph: &mut DocumentGraph,
    footnotes: &FootnoteCatalog,
    referenced_footnotes: &mut HashSet<u32>,
) -> TableState {
    let mut table = TableState::default();
    for row in table_element
        .children
        .iter()
        .filter_map(XmlValue::as_element)
        .filter(|child| element_is(child, "tr"))
    {
        let mut current_row = Vec::new();
        for cell in row
            .children
            .iter()
            .filter_map(XmlValue::as_element)
            .filter(|child| element_is(child, "tc"))
        {
            let mut current_cell = Vec::new();
            for paragraph in cell
                .children
                .iter()
                .filter_map(XmlValue::as_element)
                .filter(|child| element_is(child, "p"))
            {
                let paragraph = parse_paragraph_element(
                    paragraph,
                    rels,
                    graph,
                    footnotes,
                    referenced_footnotes,
                );
                if let Some(content) = paragraph_inlines(&paragraph) {
                    append_cell_paragraph(&mut current_cell, &content);
                }
            }
            current_row.push(current_cell);
        }
        if !current_row.is_empty() {
            table.rows.push(current_row);
        }
    }
    table
}

fn parse_paragraph_element(
    paragraph_element: &XmlElement,
    rels: &HashMap<String, String>,
    graph: &mut DocumentGraph,
    footnotes: &FootnoteCatalog,
    referenced_footnotes: &mut HashSet<u32>,
) -> ParagraphState {
    let mut paragraph = ParagraphState::default();
    if let Some(p_pr) = child_element(paragraph_element, "pPr") {
        if let Some(p_style) = child_element(p_pr, "pStyle") {
            paragraph.style = attribute_value(p_style, "val");
        }
        if let Some(num_pr) = child_element(p_pr, "numPr") {
            paragraph.is_list_item = true;
            if let Some(num_id) = child_element(num_pr, "numId") {
                paragraph.num_id = u32_attribute(num_id, "val");
            }
            if let Some(ilvl) = child_element(num_pr, "ilvl") {
                paragraph.ilvl = u32_attribute(ilvl, "val");
            }
        }
    }

    for child in paragraph_element
        .children
        .iter()
        .filter_map(XmlValue::as_element)
    {
        if element_is(child, "hyperlink") {
            let link = parse_hyperlink_element(child, graph, footnotes, referenced_footnotes);
            paragraph.push_hyperlink(&link, rels, graph);
        } else if element_is(child, "r") {
            if let Some(footnote_ref) = child_element(child, "footnoteReference") {
                push_footnote_reference(
                    graph,
                    &mut paragraph.inlines,
                    footnotes,
                    referenced_footnotes,
                    u32_attribute(footnote_ref, "id"),
                );
            } else {
                paragraph.push_run(&parse_run_element(child));
            }
        } else if element_is(child, "footnoteReference") {
            push_footnote_reference(
                graph,
                &mut paragraph.inlines,
                footnotes,
                referenced_footnotes,
                u32_attribute(child, "id"),
            );
        }
    }

    let doc_prs = elements_by_local_name(paragraph_element, "docPr");
    for (index, blip) in elements_by_local_name(paragraph_element, "blip")
        .iter()
        .enumerate()
    {
        if let Some(rel_id) = attribute_value(blip, "embed") {
            paragraph.pending_image_alt = doc_prs
                .get(index)
                .and_then(|doc_pr| attribute_value(doc_pr, "descr"));
            paragraph.push_image(&rel_id, rels, graph);
        }
    }

    paragraph
}

fn parse_hyperlink_element(
    element: &XmlElement,
    graph: &mut DocumentGraph,
    footnotes: &FootnoteCatalog,
    referenced_footnotes: &mut HashSet<u32>,
) -> HyperlinkState {
    let mut link = HyperlinkState {
        rel_id: attribute_value(element, "id"),
        inlines: Vec::new(),
    };
    for child in element.children.iter().filter_map(XmlValue::as_element) {
        if element_is(child, "r") {
            if let Some(footnote_ref) = child_element(child, "footnoteReference") {
                push_footnote_reference(
                    graph,
                    &mut link.inlines,
                    footnotes,
                    referenced_footnotes,
                    u32_attribute(footnote_ref, "id"),
                );
            } else {
                link.push_run(&parse_run_element(child));
            }
        } else if element_is(child, "footnoteReference") {
            push_footnote_reference(
                graph,
                &mut link.inlines,
                footnotes,
                referenced_footnotes,
                u32_attribute(child, "id"),
            );
        }
    }
    link
}

fn parse_run_element(element: &XmlElement) -> RunState {
    let mut run = RunState::default();
    if let Some(r_pr) = child_element(element, "rPr") {
        if let Some(bold) = child_element(r_pr, "b") {
            run.bold = bool_from_element(Some(bold), true);
        }
        if let Some(italic) = child_element(r_pr, "i") {
            run.italic = bool_from_element(Some(italic), true);
        }
    }
    for child in &element.children {
        match child {
            XmlValue::Element(child) if element_is(child, "t") => {
                run.text.push_str(&element_text(child));
            }
            XmlValue::Element(child) if element_is(child, "tab") => run.text.push('\t'),
            XmlValue::Element(child) if element_is(child, "br") => run.text.push('\n'),
            _ => {}
        }
    }
    run
}

#[derive(Debug, Default)]
struct ParagraphState {
    style: Option<String>,
    inlines: Vec<Inline>,
    pending_image_alt: Option<String>,
    is_list_item: bool,
    num_id: Option<u32>,
    ilvl: Option<u32>,
}

#[derive(Debug, Default)]
struct BodyState {
    pending_list: Option<PendingListState>,
}

#[derive(Debug)]
struct PendingListState {
    num_id: Option<u32>,
    ordered: Option<bool>,
    items: Vec<ListItem>,
}

impl Default for PendingListState {
    fn default() -> Self {
        Self {
            num_id: None,
            ordered: None,
            items: Vec::new(),
        }
    }
}

impl ParagraphState {
    fn push_run(&mut self, run: &RunState) {
        if run.text.is_empty() {
            return;
        }
        let inline = run.inline();
        push_inline(&mut self.inlines, inline);
    }

    fn push_hyperlink(
        &mut self,
        link: &HyperlinkState,
        rels: &HashMap<String, String>,
        graph: &mut DocumentGraph,
    ) {
        let display = link
            .inlines
            .iter()
            .map(inline_plain_text)
            .collect::<String>();
        if display.is_empty() {
            return;
        }
        let url = link.rel_id.as_ref().and_then(|id| rels.get(id));
        if let Some(url) = url {
            push_inline(
                &mut self.inlines,
                Inline::Styled {
                    style: "link".into(),
                    children: vec![
                        Inline::Text { text: display },
                        Inline::Text { text: url.clone() },
                    ],
                },
            );
            return;
        }
        graph.push_loss(LossMarker {
            code: "reader.docx.unresolved_hyperlink".into(),
            message: format!(
                "hyperlink relationship {:?} could not be resolved",
                link.rel_id
            ),
            status: SemanticStatus::Unresolved,
        });
        for inline in &link.inlines {
            push_inline(&mut self.inlines, inline.clone());
        }
    }

    fn push_image(
        &mut self,
        rel_id: &str,
        rels: &HashMap<String, String>,
        graph: &mut DocumentGraph,
    ) {
        let alt = self.pending_image_alt.clone().unwrap_or_default();
        self.pending_image_alt = None;
        let target = rels.get(rel_id);
        if let Some(target) = target {
            let asset_id = AssetId(graph.assets.len() as u64 + 1);
            graph.push_asset(Asset {
                id: asset_id,
                kind: AssetKind::Image,
                content_identity: None,
                source: Some(target.clone()),
                media_type: media_type_for(target),
                status: SemanticStatus::Resolved,
                bytes: None,
            });
            push_inline(
                &mut self.inlines,
                Inline::Styled {
                    style: "image".into(),
                    children: vec![
                        Inline::Text { text: alt },
                        Inline::Text { text: target.clone() },
                    ],
                },
            );
            return;
        }
        graph.push_loss(LossMarker {
            code: "reader.docx.unresolved_image".into(),
            message: format!("image relationship {rel_id} could not be resolved"),
            status: SemanticStatus::Unresolved,
        });
    }
}

#[derive(Debug, Default)]
struct HyperlinkState {
    rel_id: Option<String>,
    inlines: Vec<Inline>,
}

impl HyperlinkState {
    fn push_run(&mut self, run: &RunState) {
        if run.text.is_empty() {
            return;
        }
        push_inline(&mut self.inlines, run.inline());
    }
}

#[derive(Debug, Default)]
struct RunState {
    bold: bool,
    italic: bool,
    text: String,
}

impl RunState {
    fn inline(&self) -> Inline {
        let text = Inline::Text {
            text: self.text.clone(),
        };
        if self.bold && self.italic {
            return Inline::Styled {
                style: "bold".into(),
                children: vec![Inline::Styled {
                    style: "italic".into(),
                    children: vec![text],
                }],
            };
        }
        if self.bold {
            return Inline::Styled {
                style: "bold".into(),
                children: vec![text],
            };
        }
        if self.italic {
            return Inline::Styled {
                style: "italic".into(),
                children: vec![text],
            };
        }
        text
    }
}

fn push_footnote_reference(
    graph: &mut DocumentGraph,
    inlines: &mut Vec<Inline>,
    footnotes: &FootnoteCatalog,
    referenced: &mut HashSet<u32>,
    id: Option<u32>,
) {
    let label = id.map(|value| value.to_string()).unwrap_or_else(|| "?".to_string());
    push_inline(
        inlines,
        Inline::Text {
            text: format!("[^{}]", label),
        },
    );
    if let Some(id) = id {
        referenced.insert(id);
        if !footnotes.contains_key(&id) {
            graph.push_loss(LossMarker {
                code: "reader.docx.footnote_body".into(),
                message: format!("footnote {label} body is not resolved yet"),
                status: SemanticStatus::Unresolved,
            });
        }
    } else {
        graph.push_loss(LossMarker {
            code: "reader.docx.footnote_body".into(),
            message: "footnote reference is missing an id".into(),
            status: SemanticStatus::Unresolved,
        });
    }
}

fn push_inline(inlines: &mut Vec<Inline>, inline: Inline) {
    if let Some(last) = inlines.last_mut() {
        if merge_text_inline(last, &inline) {
            return;
        }
    }
    inlines.push(inline);
}

fn merge_text_inline(existing: &mut Inline, incoming: &Inline) -> bool {
    match (existing, incoming) {
        (Inline::Text { text: left }, Inline::Text { text: right }) => {
            left.push_str(right);
            true
        }
        _ => false,
    }
}

fn media_type_for(path: &str) -> Option<String> {
    let extension = path.rsplit('.').next()?.to_ascii_lowercase();
    match extension.as_str() {
        "png" => Some("image/png".into()),
        "jpg" | "jpeg" => Some("image/jpeg".into()),
        "gif" => Some("image/gif".into()),
        "webp" => Some("image/webp".into()),
        "svg" => Some("image/svg+xml".into()),
        _ => None,
    }
}

fn finish_paragraph(
    graph: &mut DocumentGraph,
    body: &mut BodyState,
    paragraph: &ParagraphState,
    numbering: &NumberingCatalog,
) {
    if paragraph.is_list_item {
        if let Some(content) = paragraph_inlines(paragraph) {
            let item = ListItem {
                content,
                children: Vec::new(),
            };
            let ordered = list_marker_ordered(paragraph, numbering);
            match &mut body.pending_list {
                Some(list) if list.num_id == paragraph.num_id => list.items.push(item),
                Some(_) => {
                    flush_pending_list(graph, body, numbering);
                    body.pending_list = Some(PendingListState {
                        num_id: paragraph.num_id,
                        ordered,
                        items: vec![item],
                    });
                }
                None => {
                    body.pending_list = Some(PendingListState {
                        num_id: paragraph.num_id,
                        ordered,
                        items: vec![item],
                    });
                }
            }
        }
        return;
    }
    flush_pending_list(graph, body, numbering);
    push_paragraph(graph, paragraph);
}

fn list_marker_ordered(paragraph: &ParagraphState, numbering: &NumberingCatalog) -> Option<bool> {
    match (paragraph.num_id, paragraph.ilvl) {
        (Some(num_id), ilvl) => numbering.is_ordered(num_id, ilvl.unwrap_or(0)),
        _ => None,
    }
}

fn flush_pending_list(
    graph: &mut DocumentGraph,
    body: &mut BodyState,
    numbering: &NumberingCatalog,
) {
    let Some(list) = body.pending_list.take() else {
        return;
    };
    if list.items.is_empty() {
        return;
    }
    let resolved = list
        .ordered
        .or_else(|| list.num_id.and_then(|num_id| numbering.is_ordered(num_id, 0)));
    let ordered = resolved.unwrap_or(false);
    if resolved.is_none() {
        graph.push_loss(LossMarker {
            code: "reader.docx.numbering_unresolved".into(),
            message: "list marker style inferred without word/numbering.xml".into(),
            status: SemanticStatus::Partial,
        });
    }
    graph.push_block(Block::List {
        ordered,
        items: list.items,
    });
}

fn paragraph_inlines(paragraph: &ParagraphState) -> Option<Vec<Inline>> {
    let trimmed = paragraph
        .inlines
        .iter()
        .map(inline_plain_text)
        .collect::<String>()
        .trim()
        .to_string();
    if trimmed.is_empty() {
        return None;
    }
    if paragraph.inlines.is_empty() {
        Some(vec![Inline::Text { text: trimmed }])
    } else {
        Some(paragraph.inlines.clone())
    }
}

fn push_paragraph(graph: &mut DocumentGraph, paragraph: &ParagraphState) {
    let Some(inlines) = paragraph_inlines(paragraph) else {
        return;
    };
    if let Some(level) = heading_level(&paragraph.style) {
        graph.push_block(Block::Section {
            level,
            title: inlines,
            children: Vec::new(),
        });
        return;
    }
    if paragraph.style.is_some() {
        graph.push_loss(LossMarker {
            code: "reader.docx.unmapped_style".into(),
            message: format!("paragraph style {:?} mapped to plain text", paragraph.style),
            status: SemanticStatus::Partial,
        });
    }
    graph.push_block(Block::Paragraph { content: inlines });
}

fn inline_plain_text(inline: &Inline) -> String {
    match inline {
        Inline::Text { text } => text.clone(),
        Inline::InlineCode { text } => text.clone(),
        Inline::Styled { children, .. } => children.iter().map(inline_plain_text).collect(),
        Inline::InlineMath { content, .. } => content.clone(),
        Inline::Reference { display, .. } => display.clone(),
    }
}

fn heading_level(style: &Option<String>) -> Option<u8> {
    let style = style.as_deref()?;
    if let Some(rest) = style.strip_prefix("Heading") {
        if let Ok(level) = rest.parse::<u8>() {
            if (1..=6).contains(&level) {
                return Some(level);
            }
        }
    }
    if style.eq_ignore_ascii_case("Title") {
        return Some(1);
    }
    None
}

/// Creates a new graph with a stable document id derived from the label.
pub fn new_graph(label: &str) -> DocumentGraph {
    DocumentGraph::new(document_id_for(label))
}

fn document_id_for(label: &str) -> DocumentId {
    let mut hash = 1u64;
    for byte in label.bytes() {
        hash = hash.wrapping_mul(31).wrapping_add(u64::from(byte));
    }
    DocumentId(hash)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::export::markdown::export_markdown;

    #[test]
    fn parses_footnote_reference_inside_run() {
        let xml = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p>
      <w:r><w:t>See</w:t></w:r>
      <w:r><w:footnoteReference w:id="1"/></w:r>
      <w:r><w:t> for details.</w:t></w:r>
    </w:p>
  </w:body>
</w:document>"#;
        let mut graph = new_graph("footnote");
        let referenced = parse_document_xml(
            xml,
            &HashMap::new(),
            &NumberingCatalog::default(),
            &FootnoteCatalog::new(),
            &mut graph,
        )
        .expect("parse document");
        assert!(referenced.contains(&1));
        let markdown = export_markdown(&graph).expect("export markdown");
        assert!(
            markdown.contains("See[^1] for details."),
            "markdown was: {markdown:?}, blocks: {:?}",
            graph.blocks
        );
    }

    #[test]
    fn parses_bold_and_italic_runs() {
        let xml = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p>
      <w:r><w:rPr><w:b/></w:rPr><w:t>Bold</w:t></w:r>
      <w:r><w:rPr><w:i/></w:rPr><w:t> italic</w:t></w:r>
    </w:p>
  </w:body>
</w:document>"#;
        let mut graph = new_graph("styled");
        parse_document_xml(
            xml,
            &HashMap::new(),
            &NumberingCatalog::default(),
            &FootnoteCatalog::new(),
            &mut graph,
        )
        .expect("parse document");
        let markdown = export_markdown(&graph).expect("export markdown");
        assert!(markdown.contains("**Bold**"));
        assert!(markdown.contains("* italic*"), "markdown was: {markdown}");
    }
}
