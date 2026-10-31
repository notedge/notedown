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
