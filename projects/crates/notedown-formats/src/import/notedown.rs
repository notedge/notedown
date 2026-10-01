use std::path::Path;

use notedown_ir::{
    Block, DocumentGraph, DocumentId, Inline, ListItem, LossMarker, SemanticStatus, TableRow,
};
use oak_core::parser::session::ParseSession;
use oak_core::{Parser, RedNode, RedTree, SourceText};
use oak_notedown::parser::element_type::NoteElementType;
use oak_notedown::lexer::token_type::NoteTokenType;
use oak_notedown::{NoteLanguage, NoteParser};

use crate::FormatError;

/// Import Notedown text from disk into `notedown-ir` via `oak-notedown`.
pub fn import_notedown(path: impl AsRef<Path>) -> Result<DocumentGraph, FormatError> {
    let path = path.as_ref();
    let text = std::fs::read_to_string(path).map_err(|error| {
        FormatError::invalid_input(format!("failed to read {}: {error}", path.display()))
    })?;
    import_notedown_bytes(&path.display().to_string(), &text)
}

/// Import Notedown text bytes into `notedown-ir` via `oak-notedown`.
pub fn import_notedown_bytes(label: &str, text: &str) -> Result<DocumentGraph, FormatError> {
    let language = NoteLanguage::default();
    let source = SourceText::new(text);
    let mut cache = ParseSession::<NoteLanguage>::default();
    let parser = NoteParser::new(&language);
    let parsed = parser.parse(&source, &[], &mut cache);
    let green_tree = parsed
        .result
        .map_err(|error| FormatError::parse("notedown", error.to_string()))?;
    lower_notedown_tree(label, green_tree, &source)
}

fn lower_notedown_tree(
    label: &str,
    green_tree: &oak_core::GreenNode<NoteLanguage>,
    source: &SourceText,
) -> Result<DocumentGraph, FormatError> {
    let root = RedNode::new(green_tree, 0);
    let mut graph = DocumentGraph::new(document_id_for(label));
    let blocks = lower_root_blocks(root, source);
    for block in blocks {
        graph.push_block(block);
    }

    if graph.blocks.is_empty() {
        graph.push_loss(LossMarker {
            code: "import.notedown.empty_document".into(),
            message: "notedown source produced no semantic blocks".into(),
            status: SemanticStatus::Partial,
        });
    } else {
        graph.push_loss(LossMarker {
            code: "import.notedown.partial_coverage".into(),
            message: "notedown import lowers headings, paragraphs, lists, code blocks, tables, and basic links via oak-notedown".into(),
            status: SemanticStatus::Partial,
        });
    }
    Ok(graph)
}

fn lower_root_blocks(node: RedNode<NoteLanguage>, source: &SourceText) -> Vec<Block> {
    let mut blocks = Vec::new();
    let mut pending_list: Option<(bool, Vec<ListItem>)> = None;

    for child in node.children() {
        if let RedTree::Node(child_node) = child {
            match child_node.element_type() {
                NoteElementType::ListItem => {
                    let ordered = list_marker_ordered(child_node.text(source).as_ref());
                    let item = ListItem {
                        content: collect_inlines(child_node, source),
                        children: Vec::new(),
                    };
                    match &mut pending_list {
                        Some((list_ordered, items)) if *list_ordered == ordered => {
                            items.push(item);
                        }
                        _ => {
                            flush_list(&mut blocks, &mut pending_list);
                            pending_list = Some((ordered, vec![item]));
                        }
                    }
                }
                NoteElementType::Paragraph => {
                    if let Some((ordered, item)) = promote_list_item(child_node, source) {
                        match &mut pending_list {
                            Some((list_ordered, items)) if *list_ordered == ordered => {
                                items.push(item);
                            }
                            _ => {
                                flush_list(&mut blocks, &mut pending_list);
                                pending_list = Some((ordered, vec![item]));
                            }
                        }
                        continue;
                    }
                    flush_list(&mut blocks, &mut pending_list);
                    if let Some(block) = promote_heading(child_node, source) {
                        blocks.push(block);
                    } else if let Some(block) = lower_paragraph(child_node, source) {
                        blocks.push(block);
                    }
                }
                other => {
                    flush_list(&mut blocks, &mut pending_list);
                    if let Some(block) = lower_block_node(child_node, source, other) {
                        blocks.push(block);
                    }
                }
            }
        }
    }
    flush_list(&mut blocks, &mut pending_list);
    blocks
}

fn flush_list(blocks: &mut Vec<Block>, pending: &mut Option<(bool, Vec<ListItem>)>) {
    if let Some((ordered, items)) = pending.take() {
        if !items.is_empty() {
            blocks.push(Block::List { ordered, items });
        }
    }
}

fn lower_block_node(
    node: RedNode<NoteLanguage>,
    source: &SourceText,
    kind: NoteElementType,
) -> Option<Block> {
    match kind {
        NoteElementType::Heading => {
            let (level, title) = heading_parts(node, source);
            Some(Block::Section {
                level,
                title: if title.is_empty() {
                    collect_inlines(node, source)
                } else {
                    vec![Inline::Text { text: title }]
                },
                children: Vec::new(),
            })
        }
        NoteElementType::Paragraph => lower_paragraph(node, source),
        NoteElementType::CodeBlock => {
            let (language, content) = extract_code_block(node, source);
            if content.is_empty() {
                None
            } else {
                Some(Block::Code { language, content })
            }
        }
        NoteElementType::Table => Some(Block::Table {
            rows: extract_table_rows(node, source),
        }),
        NoteElementType::Blockquote => Some(Block::Quote {
            content: collect_inlines(node, source),
        }),
        NoteElementType::HorizontalRule => Some(Block::Opaque {
            kind: "thematic_break".into(),
            payload_hint: "---".into(),
            status: SemanticStatus::Partial,
        }),
        NoteElementType::Error => None,
        _ => None,
    }
}

fn promote_heading(node: RedNode<NoteLanguage>, source: &SourceText) -> Option<Block> {
    let text = node.text(source).into_owned();
    let raw = text.trim();
    if !raw.starts_with('#') {
        return None;
    }
    let hashes = raw.chars().take_while(|ch| *ch == '#').count();
    if hashes == 0 || hashes > 6 {
        return None;
    }
    let title = raw[hashes..].trim_start();
    if title.is_empty() {
        return None;
    }
    Some(Block::Section {
        level: hashes as u8,
        title: vec![Inline::Text { text: title.to_string() }],
        children: Vec::new(),
    })
}

fn promote_list_item(
    node: RedNode<NoteLanguage>,
    source: &SourceText,
) -> Option<(bool, ListItem)> {
    let text = node.text(source).into_owned();
    let raw = text.trim();
    if let Some(content) = raw.strip_prefix("- ")
        .or_else(|| raw.strip_prefix("* "))
        .or_else(|| raw.strip_prefix("+ "))
    {
        return Some((
            false,
            ListItem {
                content: vec![Inline::Text {
                    text: content.trim().to_string(),
                }],
                children: Vec::new(),
            },
        ));
    }
    if let Some((prefix, content)) = raw.split_once(". ") {
        if !prefix.is_empty()
            && prefix
                .chars()
                .all(|ch| ch.is_ascii_digit())
            && !content.is_empty()
        {
            return Some((
                true,
                ListItem {
                    content: vec![Inline::Text {
                        text: content.trim().to_string(),
                    }],
                    children: Vec::new(),
                },
            ));
        }
    }
    let inlines = collect_inlines(node, source);
    if inlines.is_empty() {
        return None;
    }
    if let Inline::Text { text } = &inlines[0] {
        if let Some(content) = text.strip_prefix("- ")
            .or_else(|| text.strip_prefix("* "))
            .or_else(|| text.strip_prefix("+ "))
        {
            return Some((
                false,
                ListItem {
                    content: vec![Inline::Text {
                        text: content.trim().to_string(),
                    }],
                    children: Vec::new(),
                },
            ));
        }
    }
    None
}

fn lower_paragraph(node: RedNode<NoteLanguage>, source: &SourceText) -> Option<Block> {
    let content = collect_inlines(node, source);
    if content.is_empty() {
        return None;
    }
    Some(Block::Paragraph { content })
}

fn heading_parts(node: RedNode<NoteLanguage>, source: &SourceText) -> (u8, String) {
    let raw = node.text(source);
    let trimmed = raw.trim_start();
    let hashes = trimmed.chars().take_while(|ch| *ch == '#').count();
    let level = hashes.clamp(1, 6) as u8;
    let title = if hashes > 0 {
        trimmed[hashes..].trim_start().to_string()
    } else {
        collect_plain_text(node, source)
    };
    (level, title)
}

fn list_marker_ordered(text: &str) -> bool {
    text
        .trim_start()
        .chars()
        .next()
        .map(|ch| ch.is_ascii_digit())
        .unwrap_or(false)
}

fn extract_code_block(
    node: RedNode<NoteLanguage>,
    source: &SourceText,
) -> (Option<String>, String) {
    let raw = node.text(source);
    let stripped = raw
        .trim()
        .trim_start_matches('`')
        .trim_end_matches('`');
    let mut lines = stripped.lines();
    let first = lines.next().unwrap_or("").trim();
    let language = if first.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
        && !first.is_empty()
        && !first.contains(' ')
    {
        Some(first.to_string())
    } else {
        None
    };
    let content = if language.is_some() {
        lines.collect::<Vec<_>>().join("\n")
    } else {
        stripped.to_string()
    };
    (language, content.trim().to_string())
}

fn extract_table_rows(node: RedNode<NoteLanguage>, source: &SourceText) -> Vec<TableRow> {
    let mut rows = Vec::new();
    for child in node.children() {
        if let RedTree::Node(row_node) = child {
            if row_node.element_type() != NoteElementType::TableRow {
                continue;
            }
            let mut cells = Vec::new();
            for cell_child in row_node.children() {
                if let RedTree::Node(cell_node) = cell_child {
                    cells.push(collect_inlines(cell_node, source));
                }
            }
            if !cells.is_empty() {
                rows.push(TableRow { cells });
            }
        }
    }
    rows
}

fn collect_inlines(node: RedNode<NoteLanguage>, source: &SourceText) -> Vec<Inline> {
    let mut inlines = Vec::new();
    for child in node.children() {
        match child {
            RedTree::Node(child_node) => {
                if let Some(inline) = lower_inline_node(child_node, source) {
                    push_inline(&mut inlines, inline);
                }
            }
            RedTree::Leaf(_) => {
                let text = child.text(source);
                if !text.trim().is_empty() {
                    push_inline(&mut inlines, Inline::Text { text: text.into_owned() });
                }
            }
        }
    }
    if inlines.is_empty() {
        let text = node.text(source).trim().to_string();
        if !text.is_empty() {
            inlines.push(Inline::Text { text });
        }
    }
    inlines
}

fn lower_inline_node(
    node: RedNode<NoteLanguage>,
    source: &SourceText,
) -> Option<Inline> {
    match node.element_type() {
        NoteElementType::Link => parse_inline_link(node, source),
        NoteElementType::Root => {
            let children = collect_inlines(node, source);
            if children.is_empty() {
                None
            } else if children.len() == 1 {
                children.first().cloned()
            } else {
                Some(Inline::Styled {
                    style: "span".into(),
                    children,
                })
            }
        }
        NoteElementType::Token(token) => match token {
            NoteTokenType::Text | NoteTokenType::HeadingText | NoteTokenType::LinkText => {
                Some(Inline::Text {
                    text: node.text(source).into_owned(),
                })
            }
            NoteTokenType::InlineCode => Some(Inline::InlineCode {
                text: node.text(source).into_owned(),
            }),
            NoteTokenType::Strong => Some(Inline::Styled {
                style: "bold".into(),
                children: collect_inlines(node, source),
            }),
            NoteTokenType::Emphasis => Some(Inline::Styled {
                style: "italic".into(),
                children: collect_inlines(node, source),
            }),
            _ => None,
        },
        _ => None,
    }
}

fn push_inline(inlines: &mut Vec<Inline>, inline: Inline) {
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

fn collect_plain_text(node: RedNode<NoteLanguage>, source: &SourceText) -> String {
    let mut text = String::new();
    for child in node.children() {
        match child {
            RedTree::Node(child_node) => text.push_str(&collect_plain_text(child_node, source)),
            RedTree::Leaf(_) => text.push_str(&child.text(source)),
        }
    }
    if text.is_empty() {
        text = node.text(source).into_owned();
    }
    text
}

fn parse_inline_link(node: RedNode<NoteLanguage>, source: &SourceText) -> Option<Inline> {
    let raw = node.text(source);
    if let Some((text, url)) = split_markdown_link(raw.as_ref()) {
        return Some(Inline::Styled {
            style: "link".into(),
            children: vec![Inline::Text { text }, Inline::Text { text: url }],
        });
    }
    let text = collect_plain_text(node, source);
    if text.is_empty() {
        None
    } else {
        Some(Inline::Text { text })
    }
}

fn split_markdown_link(raw: &str) -> Option<(String, String)> {
    let start = raw.find("](")?;
    let text = raw[..start].trim_start_matches('[').trim().to_string();
    let url = raw[start + 2..]
        .trim_end_matches(')')
        .trim()
        .to_string();
    if text.is_empty() || url.is_empty() {
        return None;
    }
    Some((text, url))
}

fn document_id_for(label: &str) -> DocumentId {
    let mut hash = 1u64;
    for byte in label.bytes() {
        hash = hash.wrapping_mul(31).wrapping_add(u64::from(byte));
    }
    DocumentId(hash)
}
