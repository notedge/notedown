use notedown_formats::{FormatDirection, FormatStatus, capabilities, import::typst::import_typst_bytes};
use notedown_ir::{Block, DocumentGraph, Inline};

fn blocks(graph: &DocumentGraph) -> &[notedown_ir::BlockNode] {
    &graph.blocks
}

fn paragraph_text(graph: &DocumentGraph) -> Vec<String> {
    blocks(graph)
        .iter()
        .filter_map(|node| match &node.block {
            Block::Paragraph { content } => Some(content.iter().map(Inline::plain_text).collect()),
            _ => None,
        })
        .collect()
}

#[test]
fn capability_reports_typst_import_as_partial() {
    let capability = capabilities().into_iter().find(|capability| capability.id == "typst").expect("typst capability");
    assert_eq!(capability.direction, FormatDirection::Import);
    assert_eq!(capability.status, FormatStatus::Partial);
}

#[test]
fn heading_becomes_section_and_tail_keeps_paragraphs() {
    let graph = import_typst_bytes("heading.typ", "= Title\n\nIntro text.\n\nSecond paragraph.\n").unwrap();
    let section = blocks(&graph)
        .iter()
        .find_map(|node| match &node.block {
            Block::Section { level, title, .. } => Some((*level, title.iter().map(Inline::plain_text).collect::<String>())),
            _ => None,
        })
        .expect("section");
    assert_eq!(section, (1, "Title".to_string()));
    let paragraphs = paragraph_text(&graph);
    assert_eq!(paragraphs, vec!["Intro text.".to_string(), "Second paragraph.".to_string()]);
}

#[test]
fn strong_and_emphasis_keep_plain_text_without_delimiters() {
    let graph = import_typst_bytes("styles.typ", "A *bold* and _soft_ word.\n").unwrap();
    let content = match &blocks(&graph)[0].block {
        Block::Paragraph { content } => content,
        other => panic!("expected paragraph, got {other:?}"),
    };
    let styles: Vec<(&str, String)> = content
        .iter()
        .filter_map(|inline| match inline {
            Inline::Styled { style, children } => Some((style.as_str(), children.iter().map(Inline::plain_text).collect())),
            _ => None,
        })
        .collect();
    assert_eq!(styles, vec![("bold", "bold".to_string()), ("italic", "soft".to_string())]);
    assert_eq!(content.iter().map(Inline::plain_text).collect::<String>(), "A bold and soft word.");
}

#[test]
fn math_keeps_authored_source() {
    let text = "Inline $x^2 + y^2$ math.\n\n$\n  x^2 + y^2\n$\n";
    let graph = import_typst_bytes("math.typ", text).unwrap();
    let inline_math: Vec<String> = blocks(&graph)
        .iter()
        .filter_map(|node| match &node.block {
            Block::Paragraph { content } => content.iter().find_map(|inline| match inline {
                Inline::InlineMath { formula } => {
                    assert_eq!(formula.language.as_deref(), Some("typst"));
                    Some(formula.source.clone())
                }
                _ => None,
            }),
            _ => None,
        })
        .collect();
    assert_eq!(inline_math, vec!["x^2 + y^2".to_string()]);
    let display_math: Vec<&str> = blocks(&graph)
        .iter()
        .filter_map(|node| match &node.block {
            Block::Math { formula } => {
                assert_eq!(formula.language.as_deref(), Some("typst"));
                Some(formula.source.as_str())
            }
            _ => None,
        })
        .collect();
    assert_eq!(display_math, vec!["x^2 + y^2"]);
}

#[test]
fn fenced_raw_becomes_code_with_language() {
    let graph = import_typst_bytes("raw.typ", "```rust\nfn main() {}\n```\n").unwrap();
    let code = blocks(&graph)
        .iter()
        .find_map(|node| match &node.block {
            Block::Code { language, content } => Some((language.clone(), content.clone())),
            _ => None,
        })
        .expect("code block");
    assert_eq!(code, (Some("rust".to_string()), "fn main() {}".to_string()));
}

#[test]
fn unordered_and_ordered_lists_lower_with_numbers() {
    let graph = import_typst_bytes("lists.typ", "- One\n- Two\n\n1. First\n2. Second\n\nafter\n").unwrap();
    let lists: Vec<&Block> = blocks(&graph)
        .iter()
        .filter_map(|node| match &node.block {
            Block::List { .. } => Some(&node.block),
            _ => None,
        })
        .collect();
    assert_eq!(lists.len(), 2);
    let Block::List { ordered: false, items } = lists[0]
    else {
        panic!("expected unordered list, got {:?}", lists[0]);
    };
    assert_eq!(
        items.iter().map(|item| item.content.iter().map(Inline::plain_text).collect::<String>()).collect::<Vec<_>>(),
        vec!["One".to_string(), "Two".to_string()]
    );
    let Block::List { ordered: true, items } = lists[1]
    else {
        panic!("expected ordered list, got {:?}", lists[1]);
    };
    assert_eq!(items.iter().map(|item| item.number).collect::<Vec<_>>(), vec![Some(1), Some(2)]);
    assert!(paragraph_text(&graph).iter().any(|text| text == "after"));
}

#[test]
fn hash_expression_stays_opaque_and_records_loss() {
    let graph = import_typst_bytes("expr.typ", "#figure(image(\"a.png\"))\n\nafter\n").unwrap();
    let opaque: Vec<&str> = blocks(&graph)
        .iter()
        .filter_map(|node| match &node.block {
            Block::Opaque { kind, payload_hint, .. } => {
                assert!(payload_hint.contains("figure"));
                Some(kind.as_str())
            }
            _ => None,
        })
        .collect();
    assert_eq!(opaque, vec!["typst.expression.figure"]);
    assert!(graph.coverage.loss.iter().any(|loss| loss.code == "import.typst.dynamic_content"));
    assert!(graph.coverage.loss.iter().any(|loss| loss.code == "import.typst.partial_coverage"));
}

#[test]
fn empty_document_records_explicit_loss() {
    let graph = import_typst_bytes("empty.typ", "").unwrap();
    assert!(graph.blocks.is_empty());
    assert!(graph.coverage.loss.iter().any(|loss| loss.code == "import.typst.empty_document"));
}

/// The single opaque block of a document, together with its authored span.
fn only_opaque(graph: &DocumentGraph) -> (&str, &str, notedown_ir::SourceSpan) {
    let opaques: Vec<_> = blocks(graph)
        .iter()
        .filter_map(|node| match &node.block {
            Block::Opaque { kind, payload_hint, .. } => Some((kind.as_str(), payload_hint.as_str(), node.id)),
            _ => None,
        })
        .collect();
    assert_eq!(opaques.len(), 1, "expected exactly one opaque block, got {opaques:?}");
    let (kind, payload, id) = opaques[0];
    let span = graph.block_span(id).expect("opaque block carries its authored span");
    (kind, payload, span)
}

fn loss_index(graph: &DocumentGraph, code: &str) -> usize {
    graph
        .coverage
        .loss
        .iter()
        .position(|loss| loss.code == code)
        .unwrap_or_else(|| panic!("missing loss `{code}`"))
}

#[test]
fn cross_line_figure_group_becomes_one_opaque_block() {
    let group = "#figure(\n  image(\"a.png\"),\n  caption: [A box],\n)";
    let text = format!("{group}\n\nafter\n");
    let graph = import_typst_bytes("figure.typ", &text).unwrap();

    let (kind, payload, span) = only_opaque(&graph);
    assert_eq!(kind, "typst.expression.figure");
    assert!(payload.contains("caption: [A box]"), "payload kept the whole group: {payload}");
    let covered = &text[span.start as usize..span.end as usize];
    assert_eq!(covered, group, "span must cover the construct from `#` through its closing bracket");

    assert_eq!(paragraph_text(&graph), vec!["after".to_string()], "construct body must never surface as body text");

    let index = loss_index(&graph, "import.typst.dynamic_content");
    assert_eq!(graph.coverage.loss[index].status, notedown_ir::SemanticStatus::Unsupported);
    assert_eq!(graph.loss_span(index), Some(span), "loss marker carries the same span as the opaque block");
}

#[test]
fn cross_line_rect_group_keeps_arguments_out_of_the_body() {
    let group = "#rect(\n  width: 2cm\n)";
    let text = format!("{group}\n\nafter\n");
    let graph = import_typst_bytes("rect.typ", &text).unwrap();

    let (kind, payload, span) = only_opaque(&graph);
    assert_eq!(kind, "typst.expression.rect");
    assert!(payload.contains("width: 2cm"), "payload kept the continuation line: {payload}");
    assert_eq!(&text[span.start as usize..span.end as usize], group);
    assert_eq!(paragraph_text(&graph), vec!["after".to_string()]);
}

#[test]
fn group_string_literal_parenthesis_does_not_close_the_group() {
    let group = "#text(\n  \"a ) b\",\n)";
    let text = format!("{group}\n\nafter\n");
    let graph = import_typst_bytes("string.typ", &text).unwrap();

    let (_, payload, span) = only_opaque(&graph);
    assert!(payload.contains("a ) b"));
    assert_eq!(
        &text[span.start as usize..span.end as usize],
        group,
        "the `)` inside the string literal must not end the group"
    );
    assert_eq!(paragraph_text(&graph), vec!["after".to_string()]);
}

#[test]
fn group_comment_parenthesis_does_not_close_the_group() {
    let line_group = "#figure(\n  // ) not the closing bracket\n  image(\"a.png\"),\n)";
    let block_group = "#rect(\n  /* ) still inside the comment */\n  width: 2cm\n)";
    let text = format!("{line_group}\n\n{block_group}\n\nafter\n");
    let graph = import_typst_bytes("comment.typ", &text).unwrap();

    let opaques: Vec<&str> = blocks(&graph)
        .iter()
        .filter_map(|node| match &node.block {
            Block::Opaque { payload_hint, .. } => Some(payload_hint.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(opaques.len(), 2, "each construct stays one opaque block, got {opaques:?}");
    assert_eq!(&text[0..line_group.len()], line_group);
    assert_eq!(&text[line_group.len() + 2..line_group.len() + 2 + block_group.len()], block_group);
    assert_eq!(paragraph_text(&graph), vec!["after".to_string()]);
}

#[test]
fn unterminated_group_is_diagnosed_instead_of_swallowed() {
    let text = "#figure(\n  image(\"a.png\"),\n";
    let graph = import_typst_bytes("unterminated.typ", text).unwrap();

    let (_, payload, span) = only_opaque(&graph);
    assert!(payload.contains("image(\"a.png\")"));
    assert_eq!(&text[span.start as usize..span.end as usize], text, "span covers the unclosed group to the end of the region");

    let index = loss_index(&graph, "import.typst.unterminated_dynamic_group");
    assert_eq!(graph.coverage.loss[index].status, notedown_ir::SemanticStatus::Unsupported);
    assert_eq!(graph.loss_span(index), Some(span));
    assert!(paragraph_text(&graph).is_empty(), "no construct fragment may leak into the body");
}
