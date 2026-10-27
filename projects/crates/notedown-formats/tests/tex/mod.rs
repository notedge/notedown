use notedown_formats::{FormatDirection, FormatStatus, capabilities, import::tex::import_tex_bytes};
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
fn capability_reports_tex_import_as_partial() {
    let capability = capabilities().into_iter().find(|capability| capability.id == "tex").expect("tex capability");
    assert_eq!(capability.direction, FormatDirection::Import);
    assert_eq!(capability.status, FormatStatus::Partial);
}

#[test]
fn sections_keep_hierarchy() {
    let text = "\\section{One}\nBody.\n\n\\subsection{Two}\n\\subsubsection{Three}\n";
    let graph = import_tex_bytes("sections.tex", text).unwrap();
    let levels: Vec<u8> = blocks(&graph)
        .iter()
        .filter_map(|node| match &node.block {
            Block::Section { level, .. } => Some(*level),
            _ => None,
        })
        .collect();
    assert_eq!(levels, vec![1, 2, 3]);
    let titles: Vec<String> = blocks(&graph)
        .iter()
        .filter_map(|node| match &node.block {
            Block::Section { title, .. } => Some(title.iter().map(Inline::plain_text).collect()),
            _ => None,
        })
        .collect();
    assert_eq!(titles, vec!["One".to_string(), "Two".to_string(), "Three".to_string()]);
    assert!(paragraph_text(&graph).iter().any(|text| text == "Body."));
}

#[test]
fn styled_inlines_map_to_style_semantics() {
    let graph = import_tex_bytes("styles.tex", "A \\textbf{bold} and \\emph{soft} word.\n").unwrap();
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
}

#[test]
fn math_keeps_authored_source() {
    let text = "Inline $x^2 + y_1$ math.\n\n$$\\int_0^1 x\\,dx$$\n";
    let graph = import_tex_bytes("math.tex", text).unwrap();
    let inline_math: Vec<String> = blocks(&graph)
        .iter()
        .filter_map(|node| match &node.block {
            Block::Paragraph { content } => content.iter().find_map(|inline| match inline {
                Inline::InlineMath { content, language } => {
                    assert_eq!(language.as_deref(), Some("latex"));
                    Some(content.clone())
                }
                _ => None,
            }),
            _ => None,
        })
        .collect();
    assert_eq!(inline_math, vec!["x^2 + y_1".to_string()]);
    let display_math: Vec<&str> = blocks(&graph)
        .iter()
        .filter_map(|node| match &node.block {
            Block::Math { content, language } => {
                assert_eq!(language.as_deref(), Some("latex"));
                Some(content.as_str())
            }
            _ => None,
        })
        .collect();
    assert_eq!(display_math.len(), 1);
    assert!(display_math[0].contains("\\int_0^1 x"));
}

#[test]
fn unknown_command_stays_opaque_and_records_loss() {
    let graph = import_tex_bytes("unknown.tex", "\\foo{bar}\n").unwrap();
    let opaque: Vec<&str> = blocks(&graph)
        .iter()
        .filter_map(|node| match &node.block {
            Block::Opaque { kind, payload_hint, .. } => {
                assert!(payload_hint.contains("\\foo"));
                Some(kind.as_str())
            }
            _ => None,
        })
        .collect();
    assert_eq!(opaque, vec!["tex.foo"]);
    assert!(!graph.coverage.complete);
    assert!(graph.coverage.loss.iter().any(|loss| loss.code == "import.tex.partial_coverage"));
}

#[test]
fn unknown_environment_stays_opaque() {
    let graph = import_tex_bytes("unknown_env.tex", "\\begin{mystery}payload\\end{mystery}\n").unwrap();
    assert!(blocks(&graph).iter().any(|node| matches!(&node.block, Block::Opaque { kind, .. } if kind == "tex.environment.mystery")));
    assert!(graph.coverage.loss.iter().any(|loss| loss.code == "import.tex.unknown_environment"));
}

#[test]
fn unordered_and_ordered_lists_lower_with_numbers() {
    let text = "\\begin{itemize}\n\\item One\n\\item Two\n\\end{itemize}\n\n\\begin{enumerate}\n\\item First\n\\item Second\n\\end{enumerate}\n";
    let graph = import_tex_bytes("lists.tex", text).unwrap();
    let lists: Vec<&Block> = blocks(&graph)
        .iter()
        .filter_map(|node| match &node.block {
            Block::List { .. } => Some(&node.block),
            _ => None,
        })
        .collect();
    assert_eq!(lists.len(), 2);
    let Block::List { ordered: false, items } = lists[0] else {
        panic!("expected unordered list, got {:?}", lists[0]);
    };
    assert_eq!(items.len(), 2);
    assert_eq!(items[0].content.iter().map(Inline::plain_text).collect::<String>(), "One");
    assert_eq!(items[1].content.iter().map(Inline::plain_text).collect::<String>(), "Two");
    assert!(items.iter().all(|item| item.number.is_none()));

    let Block::List { ordered: true, items } = lists[1] else {
        panic!("expected ordered list, got {:?}", lists[1]);
    };
    assert_eq!(items.iter().map(|item| item.number).collect::<Vec<_>>(), vec![Some(1), Some(2)]);
}

#[test]
fn nested_list_becomes_list_item_child() {
    let text = "\\begin{itemize}\n\\item Outer\n  \\begin{enumerate}\n  \\item Inner\n  \\end{enumerate}\n\\end{itemize}\n";
    let graph = import_tex_bytes("nested.tex", text).unwrap();
    let outer = blocks(&graph)
        .iter()
        .find_map(|node| match &node.block {
            Block::List { ordered: false, items } => Some(items),
            _ => None,
        })
        .expect("outer unordered list");
    assert_eq!(outer.len(), 1);
    assert_eq!(outer[0].children.len(), 1);
    let nested = graph.block(outer[0].children[0]).expect("nested list");
    let Block::List { ordered: true, items: nested_items } = &nested.block else {
        panic!("expected nested list, got {:?}", nested.block);
    };
    assert_eq!(nested_items.len(), 1);
    assert_eq!(nested_items[0].content.iter().map(Inline::plain_text).collect::<String>(), "Inner");
}

#[test]
fn simple_tabular_becomes_table() {
    let text = "\\begin{tabular}{lc}\nName & Value \\\\\nA & 1 \\\\\n\\end{tabular}\n";
    let graph = import_tex_bytes("table.tex", text).unwrap();
    let table = blocks(&graph)
        .iter()
        .find_map(|node| match &node.block {
            Block::Table { rows, .. } => Some(rows),
            _ => None,
        })
        .expect("table block");
    assert_eq!(table.len(), 2);
    assert_eq!(table[0].cells.len(), 2);
    assert_eq!(table[0].cells[0].content.iter().map(Inline::plain_text).collect::<String>(), "Name");
    assert_eq!(table[1].cells[1].content.iter().map(Inline::plain_text).collect::<String>(), "1");
}

#[test]
fn verbatim_and_verb_become_code() {
    let text = "\\begin{verbatim}\nlet x = 1;\n\\end{verbatim}\n\n\\verb|inline|\n";
    let graph = import_tex_bytes("code.tex", text).unwrap();
    let code: Vec<&str> = blocks(&graph)
        .iter()
        .filter_map(|node| match &node.block {
            Block::Code { content, .. } => Some(content.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(code.len(), 2);
    assert_eq!(code[0], "let x = 1;");
    assert_eq!(code[1], "inline");
}

#[test]
fn includegraphics_becomes_image_and_registers_asset() {
    let graph = import_tex_bytes("image.tex", "\\includegraphics[width=3cm]{figure.png}\n").unwrap();
    let images: Vec<&Inline> = blocks(&graph)
        .iter()
        .flat_map(|node| match &node.block {
            Block::Paragraph { content } => content.iter().collect::<Vec<_>>(),
            _ => Vec::new(),
        })
        .filter(|inline| matches!(inline, Inline::Image { .. }))
        .collect();
    assert_eq!(images.len(), 1);
    let Inline::Image { source, asset, .. } = images[0] else {
        unreachable!()
    };
    assert_eq!(source, "figure.png");
    assert!(asset.is_some());
    assert_eq!(graph.assets.len(), 1);
}

#[test]
fn left_right_pairs_keep_authored_delimiters() {
    let text = "\\left( x + y \\right) \\left[ a \\right] \\left\\{ b \\right\\}\n";
    let graph = import_tex_bytes("delimiters.tex", text).unwrap();
    let pairs: Vec<(String, String, String)> = paragraph_inlines(&graph)
        .into_iter()
        .filter_map(delimiter_pair)
        .map(|(left, right, children)| (left.to_string(), right.to_string(), children.iter().map(Inline::plain_text).collect()))
        .collect();
    assert_eq!(
        pairs,
        vec![
            ("(".to_string(), ")".to_string(), "x+y".to_string()),
            ("[".to_string(), "]".to_string(), "a".to_string()),
            ("\\{".to_string(), "\\}".to_string(), "b".to_string()),
        ]
    );
}

#[test]
fn nested_left_right_pairs_keep_inner_structure() {
    let graph = import_tex_bytes("nested_delimiters.tex", "\\left( \\left[ x \\right] \\right)\n").unwrap();
    let inlines = paragraph_inlines(&graph);
    let outer = inlines.iter().find_map(|inline| delimiter_pair(inline)).expect("outer delimiter pair");
    assert_eq!((outer.0, outer.1), ("(", ")"));
    let inner = outer.2.iter().find_map(delimiter_pair).expect("inner delimiter pair");
    assert_eq!((inner.0, inner.1), ("[", "]"));
    assert_eq!(inner.2.iter().map(Inline::plain_text).collect::<String>(), "x");
    // The inner pair stays its own node instead of flattening into the outer body
    // text.
    assert!(matches!(outer.2, [Inline::Attributed { .. }]));
}

#[test]
fn limits_marker_stays_adjacent_to_its_operator() {
    let graph = import_tex_bytes("limits.tex", "\\sum\\limits_{i=1}^{n}\n").unwrap();
    assert!(
        blocks(&graph)
            .iter()
            .any(|node| matches!(&node.block, Block::Opaque { kind, payload_hint, .. } if kind == "tex.sum" && payload_hint == "\\sum"))
    );

    let inlines = paragraph_inlines(&graph);
    let marker = inlines.first().expect("limits marker inline");
    assert_eq!(limits_value(marker), Some("limits"));
    assert!(matches!(marker, Inline::Attributed { children, .. } if children.is_empty()));

    let scripts: Vec<(&str, String)> = inlines[1..]
        .iter()
        .filter_map(|inline| match inline {
            Inline::Styled { style, children } => Some((style.as_str(), children.iter().map(Inline::plain_text).collect())),
            _ => None,
        })
        .collect();
    assert_eq!(scripts, vec![("subscript", "i=1".to_string()), ("superscript", "n".to_string())]);
    assert!(graph.coverage.loss.iter().any(|loss| loss.code == "import.tex.limits_marker"));
}

#[test]
fn nolimits_marker_records_the_nolimits_variant() {
    let graph = import_tex_bytes("nolimits.tex", "\\int\\nolimits_0^1\n").unwrap();
    assert!(
        blocks(&graph)
            .iter()
            .any(|node| matches!(&node.block, Block::Opaque { kind, payload_hint, .. } if kind == "tex.int" && payload_hint == "\\int"))
    );

    let inlines = paragraph_inlines(&graph);
    let marker = inlines.first().expect("nolimits marker inline");
    assert_eq!(limits_value(marker), Some("nolimits"));

    let scripts: Vec<(&str, String)> = inlines[1..]
        .iter()
        .filter_map(|inline| match inline {
            Inline::Styled { style, children } => Some((style.as_str(), children.iter().map(Inline::plain_text).collect())),
            _ => None,
        })
        .collect();
    assert_eq!(scripts, vec![("subscript", "0".to_string()), ("superscript", "1".to_string())]);
}

/// Inline content of every paragraph block in document order.
fn paragraph_inlines(graph: &DocumentGraph) -> Vec<&Inline> {
    blocks(graph)
        .iter()
        .filter_map(|node| match &node.block {
            Block::Paragraph { content } => Some(content.iter()),
            _ => None,
        })
        .flatten()
        .collect()
}

/// Authored `\left...\right` delimiters plus the lowered body of the pair.
fn delimiter_pair(inline: &Inline) -> Option<(&str, &str, &[Inline])> {
    let Inline::Attributed { attributes, children } = inline else {
        return None;
    };
    if !attributes.classes.iter().any(|class| class == "tex-delimiter-pair") {
        return None;
    }
    let value = |key: &str| attributes.pairs.iter().find(|pair| pair.key == key).map(|pair| pair.value.as_str());
    Some((value("data-tex-left")?, value("data-tex-right")?, children.as_slice()))
}

/// Authored `\limits` / `\nolimits` variant recorded by the marker inline.
fn limits_value(inline: &Inline) -> Option<&str> {
    let Inline::Attributed { attributes, .. } = inline else {
        return None;
    };
    if !attributes.classes.iter().any(|class| class == "tex-limits") {
        return None;
    }
    attributes.pairs.iter().find(|pair| pair.key == "data-tex-limits").map(|pair| pair.value.as_str())
}
