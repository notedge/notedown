use notedown_ir::{
    Block, DocumentGraph, Inline, ListItem, LossMarker, SemanticStatus, TableRow,
};

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
    let (mut blocks, mut losses) =
        blocks_from_html_document(&document, graph, member_path);

    let scraped = scraper_blocks_from_xhtml(text.as_str(), graph, member_path)?;
    blocks = merge_xhtml_blocks(blocks, scraped);

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

pub(crate) fn merge_xhtml_blocks(oak: Vec<Block>, scraped: Vec<Block>) -> Vec<Block> {
    if scraped.is_empty() {
        return oak;
    }
    if oak.is_empty() {
        return scraped;
    }
    let oak_score = semantic_block_score(&oak);
    let scraped_score = semantic_block_score(&scraped);
    let oak_quality = semantic_block_quality(&oak);
    let scraped_quality = semantic_block_quality(&scraped);
    if scraped_quality > oak_quality {
        scraped
    } else if oak_quality > scraped_quality {
        oak
    } else if scraped_score > oak_score
        || (scraped_score == oak_score && scraped.len() >= oak.len())
    {
        scraped
    } else {
        oak
    }
}

/// Penalize blocks that still contain unparsed markup in text payloads.
fn semantic_block_quality(blocks: &[Block]) -> isize {
    blocks
        .iter()
        .map(|block| block_quality(block) as isize)
        .sum()
}

fn block_quality(block: &Block) -> isize {
    let mut score = block_score(block) as isize;
    if block_contains_unparsed_markup(block) {
        score -= 1_000;
    }
    score
}

fn block_contains_unparsed_markup(block: &Block) -> bool {
    match block {
        Block::Paragraph { content } | Block::Quote { content } | Block::Section { title: content, .. } => {
            content.iter().any(|inline| inline_contains_unparsed_markup(inline))
        }
        Block::Code { content, .. } => content.contains('<') && content.contains('>'),
        Block::List { items, .. } => items
            .iter()
            .any(|item| item.content.iter().any(inline_contains_unparsed_markup)),
        Block::Table { rows, .. } => rows.iter().any(|row| {
            row.cells
                .iter()
                .any(|cell| cell.iter().any(inline_contains_unparsed_markup))
        }),
        _ => false,
    }
}

fn inline_contains_unparsed_markup(inline: &Inline) -> bool {
    match inline {
        Inline::Text { text } => text.contains('<') && text.contains('>'),
        Inline::Styled { children, .. } => children.iter().any(inline_contains_unparsed_markup),
        _ => false,
    }
}

fn semantic_block_score(blocks: &[Block]) -> usize {
    blocks.iter().map(block_score).sum()
}

fn block_score(block: &Block) -> usize {
    match block {
        Block::List { items, .. } => {
            10 + items.len()
                + items
                    .iter()
                    .map(|item| 5 + item.content.len() + item.children.len() * 8)
                    .sum::<usize>()
        }
        Block::Table { rows, .. } => 8 + rows.len(),
        Block::Section { title, .. } => 4 + title.len(),
        Block::Paragraph { content, .. } => 2 + content.len(),
        Block::Quote { content, .. } => 3 + content.len(),
        Block::Code { content, .. } => 3 + content.len().min(32),
        Block::Opaque { kind, .. } if kind == "thematic_break" => 2,
        _ => 1,
    }
}

/// Fallback block extraction when `oak-html` tree shape is still lossy.
fn scraper_blocks_from_xhtml(
    text: &str,
    graph: &mut DocumentGraph,
    member_path: Option<&str>,
) -> Result<Vec<Block>, FormatError> {
    let mut blocks = Vec::new();
    let mut cursor = 0usize;

    while let Some(start) = text[cursor..].find('<') {
        let absolute = cursor + start;
        let rest = &text[absolute + 1..];
        let (tag, is_close) = parse_tag_name(rest)?;
        let close_index = rest
            .find('>')
            .ok_or_else(|| FormatError::parse("epub", "malformed xhtml tag"))?;
        let tag_source = &text[absolute..absolute + 1 + close_index + 1];
        let inner_start = absolute + 1 + close_index + 1;

        if is_close {
            cursor = inner_start;
            continue;
        }

        if tag == "img" {
            let src = scrape_attribute(tag_source, "src").unwrap_or_default();
            let alt = scrape_attribute(tag_source, "alt").unwrap_or_default();
            if let Some(inline) = register_and_image_inline(&src, &alt, graph, member_path) {
                blocks.push(Block::Paragraph {
                    content: vec![inline],
                });
            }
            cursor = inner_start;
            continue;
        }

        if tag == "svg" {
            let (content, next) = slice_until_close(text, inner_start, "svg")?;
            if let Some((href, alt)) = scrape_svg_image_reference(content) {
                if let Some(inline) = register_and_image_inline(&href, &alt, graph, member_path) {
                    blocks.push(Block::Paragraph {
                        content: vec![inline],
                    });
                }
            }
            cursor = next;
            continue;
        }

        if matches!(tag.as_str(), "h1" | "h2" | "h3" | "h4" | "h5" | "h6") {
            let level = tag[1..].parse::<u8>().unwrap_or(1);
            let (content, next) = slice_until_close(text, inner_start, &tag)?;
            blocks.push(Block::Section {
                level,
                title: vec![Inline::Text {
                    text: strip_tags(content),
                }],
                children: Vec::new(),
            });
            cursor = next;
            continue;
        }

        if tag == "p" {
            let (content, next) = slice_until_close(text, inner_start, "p")?;
            let inlines = inlines_from_html_fragment(content, graph, member_path);
            if !inlines.is_empty() {
                blocks.push(Block::Paragraph { content: inlines });
            }
            cursor = next;
            continue;
        }

        if tag == "figcaption" {
            let (content, next) = slice_until_close(text, inner_start, "figcaption")?;
            let inlines = inlines_from_html_fragment(content, graph, member_path);
            let children = if inlines.is_empty() {
                let plain = strip_tags(content);
                if plain.is_empty() {
                    cursor = next;
                    continue;
                }
                vec![Inline::Text { text: plain }]
            } else {
                inlines
            };
            blocks.push(Block::Paragraph {
                content: vec![Inline::Styled {
                    style: "figcaption".into(),
                    children,
                }],
            });
            cursor = next;
            continue;
        }

        if tag == "ul" || tag == "ol" {
            let (content, next) = slice_until_close(text, inner_start, &tag)?;
            let items = scrape_list_items(content, graph, member_path);
            if !items.is_empty() {
                blocks.push(Block::List {
                    ordered: tag == "ol",
                    items,
                });
            }
            cursor = next;
            continue;
        }

        if matches!(
            tag.as_str(),
            "html" | "body" | "section" | "article" | "main" | "div" | "figure"
        ) {
            let (content, next) = slice_until_close(text, inner_start, &tag)?;
            blocks.extend(scraper_blocks_from_xhtml(content, graph, member_path)?);
            cursor = next;
            continue;
        }

        if tag == "blockquote" {
            let (content, next) = slice_until_close(text, inner_start, "blockquote")?;
            let inner = scraper_blocks_from_xhtml(content, graph, member_path)?;
            blocks.push(if inner.is_empty() {
                Block::Quote {
                    content: vec![Inline::Text {
                        text: strip_tags(content),
                    }],
                }
            } else {
                quote_block_from_blocks(inner)
            });
            cursor = next;
            continue;
        }

        if tag == "pre" {
            let (content, next) = slice_until_close(text, inner_start, "pre")?;
            let (language, code) = scrape_pre_code(content);
            if !code.is_empty() {
                blocks.push(Block::Code {
                    language,
                    content: code,
                });
            }
            cursor = next;
            continue;
        }

        if tag == "hr" {
            blocks.push(Block::Opaque {
                kind: "thematic_break".into(),
                payload_hint: "---".into(),
                status: SemanticStatus::Partial,
            });
            cursor = inner_start;
            continue;
        }

        if tag == "table" {
            let (content, next) = slice_until_close(text, inner_start, "table")?;
            let rows = scrape_table_rows(content, graph, member_path);
            if !rows.is_empty() {
                blocks.push(Block::Table { rows });
            }
            cursor = next;
            continue;
        }

        cursor = inner_start;
    }

    Ok(blocks)
}

fn scrape_list_items(
    content: &str,
    graph: &mut DocumentGraph,
    member_path: Option<&str>,
) -> Vec<ListItem> {
    let mut items = Vec::new();
    let mut cursor = 0usize;
    while let Some(start) = find_tag_open(&content[cursor..], "li") {
        let absolute = cursor + start;
        let rest = &content[absolute + 1..];
        let close_index = rest.find('>').unwrap_or(0);
        let inner_start = absolute + 1 + close_index + 1;
        let (inner, next) = slice_until_close(content, inner_start, "li").unwrap_or((
            &content[inner_start..],
            content.len(),
        ));
        let nested_lists = scrape_nested_lists(inner, graph, member_path);
        let inline_content = remove_nested_lists(inner);
        let inlines = inlines_from_html_fragment(&inline_content, graph, member_path);
        items.push(ListItem {
            content: if inlines.is_empty() {
                vec![Inline::Text {
                    text: strip_tags(inner),
                }]
            } else {
                inlines
            },
            children: nested_lists,
        });
        cursor = next;
    }
    items
}

fn remove_nested_lists(fragment: &str) -> String {
    let mut parts = Vec::new();
    let mut cursor = 0usize;
    while cursor < fragment.len() {
        let next_ul = fragment[cursor..].find("<ul");
        let next_ol = fragment[cursor..].find("<ol");
        let next_special = match (next_ul, next_ol) {
            (Some(left), Some(right)) => cursor + left.min(right),
            (Some(left), None) => cursor + left,
            (None, Some(right)) => cursor + right,
            (None, None) => fragment.len(),
        };
        parts.push(&fragment[cursor..next_special]);
        if next_special >= fragment.len() {
            break;
        }
        let tag = if fragment[next_special..].starts_with("<ul") {
            "ul"
        } else {
            "ol"
        };
        let rest = &fragment[next_special + 1..];
        let close_index = rest.find('>').unwrap_or(0);
        let inner_start = next_special + 1 + close_index + 1;
        if let Ok((_, next)) = slice_until_close(fragment, inner_start, tag) {
            cursor = next;
        } else {
            parts.push(&fragment[next_special..next_special + 1]);
            cursor = next_special + 1;
        }
    }
    parts.join("")
}

fn scrape_nested_lists(
    content: &str,
    graph: &mut DocumentGraph,
    member_path: Option<&str>,
) -> Vec<notedown_ir::NodeId> {
    let mut children = Vec::new();
    let mut cursor = 0usize;
    while cursor < content.len() {
        let next_ul = content[cursor..].find("<ul");
        let next_ol = content[cursor..].find("<ol");
        let (absolute, tag, ordered) = match (next_ul, next_ol) {
            (Some(left), Some(right)) if left <= right => (cursor + left, "ul", false),
            (Some(_), Some(right)) => (cursor + right, "ol", true),
            (Some(left), None) => (cursor + left, "ul", false),
            (None, Some(right)) => (cursor + right, "ol", true),
            (None, None) => break,
        };
        let rest = &content[absolute + 1..];
        let close_index = rest.find('>').unwrap_or(0);
        let inner_start = absolute + 1 + close_index + 1;
        if let Ok((inner, next)) = slice_until_close(content, inner_start, tag) {
            let items = scrape_list_items(inner, graph, member_path);
            if !items.is_empty() {
                children.push(graph.push_block(Block::List { ordered, items }));
            }
            cursor = next;
        } else {
            cursor = absolute + 1;
        }
    }
    children
}

fn scrape_table_rows(
    content: &str,
    graph: &mut DocumentGraph,
    member_path: Option<&str>,
) -> Vec<TableRow> {
    let mut rows = Vec::new();
    let mut cursor = 0usize;
    while let Some(start) = content[cursor..].find("<tr") {
        let absolute = cursor + start;
        let rest = &content[absolute + 1..];
        let close_index = rest.find('>').unwrap_or(0);
        let inner_start = absolute + 1 + close_index + 1;
        let (inner, next) = slice_until_close(content, inner_start, "tr").unwrap_or((
            &content[inner_start..],
            content.len(),
        ));
        let mut cells = Vec::new();
        let mut cell_cursor = 0usize;
        while let Some(cell_start) = inner[cell_cursor..].find('<') {
            let cell_absolute = cell_cursor + cell_start;
            let cell_rest = &inner[cell_absolute + 1..];
            let (cell_tag, _) = parse_tag_name(cell_rest).unwrap_or((String::new(), false));
            if cell_tag != "th" && cell_tag != "td" {
                cell_cursor = cell_absolute + 1;
                continue;
            }
            let cell_close = cell_rest.find('>').unwrap_or(0);
            let cell_inner_start = cell_absolute + 1 + cell_close + 1;
            let (cell_inner, cell_next) = slice_until_close(inner, cell_inner_start, &cell_tag)
                .unwrap_or((&inner[cell_inner_start..], inner.len()));
            cells.push(inlines_from_html_fragment(cell_inner, graph, member_path));
            cell_cursor = cell_next;
        }
        if !cells.is_empty() {
            rows.push(TableRow { cells });
        }
        cursor = next;
    }
    rows
}

fn scrape_attribute(tag: &str, name: &str) -> Option<String> {
    let lower = tag.to_ascii_lowercase();
    let needle = format!("{name}=");
    let start = lower.find(&needle)?;
    let value_start = start + needle.len();
    let quote = tag[value_start..].chars().next()?;
    if quote != '"' && quote != '\'' {
        return None;
    }
    let rest = &tag[value_start + 1..];
    let end = rest.find(quote)?;
    Some(rest[..end].to_string())
}

fn inlines_from_html_fragment(
    content: &str,
    graph: &mut DocumentGraph,
    member_path: Option<&str>,
) -> Vec<Inline> {
    let mut inlines = Vec::new();
    let mut cursor = 0usize;
    while let Some(start) = content[cursor..].find('<') {
        let absolute = cursor + start;
        let rest = &content[absolute + 1..];
        if rest.starts_with('/') {
            cursor = absolute + 1;
            continue;
        }
        let (tag, _) = parse_tag_name(rest).unwrap_or((String::new(), false));
        let close_index = rest.find('>').unwrap_or(0);
        let inner_start = absolute + 1 + close_index + 1;
        if tag == "img" {
            let tag_source = &content[absolute..inner_start];
            let src = scrape_attribute(tag_source, "src").unwrap_or_default();
            let alt = scrape_attribute(tag_source, "alt").unwrap_or_default();
            if let Some(inline) = register_and_image_inline(&src, &alt, graph, member_path) {
                push_inline(&mut inlines, inline);
            }
            cursor = inner_start;
            continue;
        }
        if matches!(tag.as_str(), "strong" | "b" | "em" | "i") {
            let (inner, next) = slice_until_close(content, inner_start, &tag)
                .unwrap_or((&content[inner_start..], content.len()));
            let style = if matches!(tag.as_str(), "strong" | "b") {
                "bold"
            } else {
                "italic"
            };
            push_inline(
                &mut inlines,
                Inline::Styled {
                    style: style.into(),
                    children: vec![Inline::Text {
                        text: strip_tags(inner),
                    }],
                },
            );
            cursor = next;
            continue;
        }
        cursor = inner_start;
    }
    let plain = strip_tags(&content[cursor..]);
    if !plain.is_empty() {
        push_inline(&mut inlines, Inline::Text { text: plain });
    }
    if inlines.is_empty() {
        let plain = strip_tags(content);
        if !plain.is_empty() {
            inlines.push(Inline::Text { text: plain });
        }
    }
    inlines
}

fn parse_tag_name(rest: &str) -> Result<(String, bool), FormatError> {
    let trimmed = rest.trim_start();
    let close = trimmed.starts_with('/');
    let name = trimmed.trim_start_matches('/');
    let end = name
        .find(|ch: char| ch == '>' || ch.is_whitespace() || ch == '/')
        .unwrap_or(name.len());
    let tag = name[..end].to_string();
    if tag.is_empty() {
        return Err(FormatError::parse("epub", "empty xhtml tag"));
    }
    Ok((tag, close))
}

fn slice_until_close<'a>(
    text: &'a str,
    start: usize,
    tag: &str,
) -> Result<(&'a str, usize), FormatError> {
    let close = format!("</{tag}>");
    let mut depth = 1usize;
    let mut cursor = start;
    while cursor < text.len() && depth > 0 {
        let rest = &text[cursor..];
        let next_open = find_tag_open(rest, tag);
        let next_close = rest.find(&close);
        match (next_open, next_close) {
            (Some(open_at), Some(close_at)) if open_at < close_at => {
                depth += 1;
                cursor += open_at + tag_open_token_len(rest, tag, open_at);
            }
            (None, Some(close_at)) => {
                depth -= 1;
                if depth == 0 {
                    let end = cursor + close_at;
                    return Ok((&text[start..end], end + close.len()));
                }
                cursor += close_at + close.len();
            }
            (Some(_open_at), Some(close_at)) => {
                depth -= 1;
                if depth == 0 {
                    let end = cursor + close_at;
                    return Ok((&text[start..end], end + close.len()));
                }
                cursor += close_at + close.len();
            }
            (Some(open_at), None) => {
                depth += 1;
                cursor += open_at + tag_open_token_len(rest, tag, open_at);
            }
            (None, None) => {
                return Err(FormatError::parse("epub", format!("unclosed <{tag}>")));
            }
        }
    }
    Err(FormatError::parse("epub", format!("unclosed <{tag}>")))
}

fn find_tag_open(rest: &str, tag: &str) -> Option<usize> {
    let needle = format!("<{tag}");
    let mut cursor = 0usize;
    while let Some(index) = rest[cursor..].find(&needle) {
        let absolute = cursor + index;
        let after = &rest[absolute + needle.len()..];
        let valid = match after.chars().next() {
            None | Some('>') | Some('/') => true,
            Some(ch) => ch.is_whitespace(),
        };
        if valid {
            return Some(absolute);
        }
        cursor = absolute + needle.len();
    }
    None
}

fn tag_open_token_len(rest: &str, tag: &str, open_at: usize) -> usize {
    let after_needle = &rest[open_at + tag.len() + 1..];
    let close = after_needle.find('>').unwrap_or(after_needle.len());
    tag.len() + 1 + close + 1
}

fn strip_tags(text: &str) -> String {
    let mut output = String::new();
    let mut in_tag = false;
    for ch in text.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => output.push(ch),
            _ => {}
        }
    }
    normalize_whitespace(output.trim())
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

fn scrape_svg_image_reference(fragment: &str) -> Option<(String, String)> {
    let lower = fragment.to_ascii_lowercase();
    let image_start = lower.find("<image")?;
    let rest = &fragment[image_start..];
    let tag_end = rest.find('>').unwrap_or(rest.len());
    let tag = &rest[..tag_end + 1];
    let href = scrape_attribute(tag, "href")
        .or_else(|| scrape_attribute(tag, "xlink:href"))?;
    if href.is_empty() {
        return None;
    }
    let alt = scrape_attribute(tag, "alt")
        .or_else(|| scrape_attribute(tag, "aria-label"))
        .unwrap_or_default();
    Some((href, alt))
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

fn scrape_pre_code(content: &str) -> (Option<String>, String) {
    if let Some(start) = find_tag_open(content, "code") {
        let rest = &content[start..];
        let close_index = rest.find('>').unwrap_or(0);
        let tag_source = &rest[..close_index + 1];
        let inner_start = start + close_index + 1;
        let language = scrape_attribute(tag_source, "class")
            .as_deref()
            .map(language_from_class);
        if let Ok((inner, _)) = slice_until_close(content, inner_start, "code") {
            return (language, strip_tags(inner));
        }
    }
    (None, strip_tags(content))
}

fn unquote(value: &str) -> String {
    let trimmed = value.trim();
    if (trimmed.starts_with('"') && trimmed.ends_with('"'))
        || (trimmed.starts_with('\'') && trimmed.ends_with('\''))
    {
        trimmed[1..trimmed.len() - 1].to_string()
    } else {
        trimmed.to_string()
    }
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
    fn scraper_lowers_blockquote_and_pre_code() {
        const XHTML: &str = r#"<blockquote><p>Quoted line</p></blockquote>
<pre><code class="language-rust">fn main() {}</code></pre><hr/>"#;
        let mut graph = notedown_ir::DocumentGraph::new(notedown_ir::DocumentId(4));
        let blocks = scraper_blocks_from_xhtml(XHTML, &mut graph, None).expect("scrape");
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
