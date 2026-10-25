use notedown_formats::{
    export::{docx::export_docx_bytes, html::export_html, markdown::export_markdown, pdf::export_pdf_bytes},
    import::{docx::import_docx_bytes, html::import_html_bytes, markdown::import_markdown_bytes, pdf::import_pdf_bytes},
};
use notedown_ir::{Block, DocumentGraph, Inline};

/// The source bodies include the fuzz boundaries a diagram fence must keep:
/// an empty body, a long body, backticks, a tilde run, a nested backtick fence,
/// and a Mermaid statement the diagram lowering does not know.
const DIAGRAM_SOURCES: &[(&str, &str)] = &[
    ("empty", ""),
    ("single-line", "flowchart TD"),
    ("ordinary", "flowchart TD\n  A[Start] --> B{Done?}\n  B -->|yes| C[Stop]"),
    ("backticks", "flowchart LR\n  A[\"`code`\"] --> B"),
    ("tilde-run", "flowchart TD\n  A[~~~] --> B"),
    ("nested-backtick-fence", "flowchart TD\n```\n  A --> B\n```"),
    ("unsupported-statement", "sequenceDiagram\n  Alice->>Bob: Hi\n  Note over Alice,Bob: unsupported"),
    ("escaped-characters", "flowchart TD\n  A[\"a & b < c > d ?\"] --> B"),
    (
        "long",
        "flowchart TD\n  N0[Alpha] --> N1[Beta]\n  N1 --> N2[Gamma]\n  N2 --> N3[Delta]\n  N3 --> N4[Epsilon]\n  N4 --> N5[Zeta]\n  N5 --> N6[Eta]\n  N6 --> N7[Theta]\n  N7 --> N8[Iota]\n  N8 --> N9[Kappa]\n  N9 --> N10[Lambda]\n  N10 --> N11[Mu]\n  N11 --> N12[Nu]",
    ),
];

fn fenced(source: &str) -> String {
    let run = source.split(|character| character == '\n').map(longest_backtick_run).max().unwrap_or(0);
    let fence = "`".repeat((run + 1).max(3));
    format!("{fence}mermaid\n{source}\n{fence}\n")
}

fn longest_backtick_run(line: &str) -> usize {
    let mut longest = 0;
    let mut current = 0;
    for character in line.chars() {
        if character == '`' {
            current += 1;
            longest = longest.max(current);
        }
        else {
            current = 0;
        }
    }
    longest
}

fn import_diagram(source: &str) -> DocumentGraph {
    import_markdown_bytes("diagram.md", &fenced(source)).expect("import diagram markdown")
}

fn diagram_block(graph: &DocumentGraph, expected_language: &str) -> String {
    let block = graph
        .blocks
        .iter()
        .map(|node| &node.block)
        .find(|block| matches!(block, Block::Diagram { .. }))
        .unwrap_or_else(|| panic!("diagram expected: {:#?}", graph.blocks));
    let Block::Diagram { language, source } = block
    else {
        unreachable!()
    };
    assert_eq!(language, expected_language);
    source.clone()
}

#[test]
fn markdown_diagram_sources_survive_markdown_and_html_round_trips() {
    for (name, source) in DIAGRAM_SOURCES {
        let graph = import_diagram(source);
        assert_eq!(diagram_block(&graph, "mermaid"), *source, "{name}");

        let markdown = export_markdown(&graph).expect("export diagram markdown");
        let reopened = import_markdown_bytes(&format!("{name}.md"), &markdown).expect("reopen diagram markdown");
        assert_eq!(diagram_block(&reopened, "mermaid"), *source, "{name}\n{markdown}");

        let html = export_html(&graph).expect("export diagram html");
        assert!(html.contains("data-diagram-language=\"mermaid\""), "{name}\n{html}");
        let from_html = import_html_bytes(&format!("{name}.html"), html.as_bytes()).expect("reopen diagram html");
        assert_eq!(diagram_block(&from_html, "mermaid"), *source, "{name}\n{html}");
    }
}

#[test]
fn docx_diagram_paragraphs_preserve_language_and_source() {
    for (name, source) in DIAGRAM_SOURCES {
        let graph = import_diagram(source);
        let bytes = export_docx_bytes(&graph).expect("export diagram docx");
        let reopened = import_docx_bytes(&format!("{name}.docx"), &bytes).expect("reopen diagram docx");
        assert_eq!(diagram_block(&reopened, "mermaid"), *source, "{name}");
    }
}

#[test]
fn pdf_diagram_export_keeps_every_source_line_intact() {
    for (name, source) in DIAGRAM_SOURCES {
        let graph = import_diagram(source);
        let bytes = export_pdf_bytes(&graph).expect("export diagram pdf");
        let reopened = import_pdf_bytes(&format!("{name}.pdf"), &bytes).expect("reopen diagram pdf");
        let text = reopened
            .blocks
            .iter()
            .filter_map(|node| match &node.block {
                Block::Paragraph { content } | Block::Quote { content } => Some(content.iter().map(Inline::plain_text).collect::<String>()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n");
        for line in source.lines().filter(|line| !line.trim().is_empty()) {
            assert!(text.contains(line.trim()), "{name}: `{line}` missing from PDF text\n{text}");
        }
    }
}
