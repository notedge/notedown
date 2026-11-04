//! The diagram entry consumes the shared rejection catalog.
//!
//! Every expected code, severity, and span comes from
//! `davinci_types::rejections`, so this test owns no private expectations.

use std::collections::BTreeSet;

use davinci_diagram::{DiagramError, LoweredDiagram};
use davinci_types::{cases, code_docs, codes};

/// Every diagram code this crate can emit.
const SHIPPED: [&str; 5] = [codes::PARSE_FAILED, codes::AST_UNAVAILABLE, codes::UNSUPPORTED, codes::APPROXIMATED, codes::EMPTY_DIAGRAM];

fn lower(language: &str, source: &str) -> Result<LoweredDiagram, DiagramError> {
    match language {
        #[cfg(feature = "mermaid")]
        "mermaid" => davinci_diagram::mermaid_to_scene(source),
        #[cfg(feature = "dot")]
        "dot" => davinci_diagram::dot_to_scene(source),
        #[cfg(feature = "plantuml")]
        "plantuml" => davinci_diagram::plantuml_to_scene(source),
        other => panic!("the catalog carries `{other}` but this build has no such frontend feature"),
    }
}

#[test]
fn the_catalog_documents_every_diagram_code_and_invents_none() {
    let docs = code_docs();
    let documented: BTreeSet<&str> = docs
        .iter()
        .map(|doc| doc.code.as_str())
        .filter(|code| code.starts_with("davinci.diagram."))
        .collect();

    for code in SHIPPED {
        assert!(documented.contains(code), "`{code}` is emitted but missing from the catalog");
    }
    for code in &documented {
        assert!(SHIPPED.contains(code), "the catalog documents `{code}`, which this crate cannot emit");
    }
}

#[test]
fn the_diagram_entry_reports_the_catalog_code_severity_and_span() {
    let cases: Vec<_> = cases().into_iter().filter(|case| case.language != "vega-lite").collect();
    assert!(!cases.is_empty(), "the catalog must carry diagram cases");

    let mut comparisons = 0;
    for case in &cases {
        // Author the fence exactly as a host would, then keep the byte offset so
        // the reported span is compared in authored bytes, not in a normalized
        // copy of the source.
        let document = format!("# Rejection\n\n```{}\n{}```\n", case.language, case.source);
        let body_start = document.find(&case.source).unwrap_or_else(|| panic!("`{}` must sit in the fence", case.id));

        let error = match lower(&case.language, &case.source) {
            Err(error) => error,
            Ok(_) => panic!("`{}` must be rejected, but the source lowered", case.id),
        };

        assert_eq!(error.code(), case.code, "`{}` reported `{}`, the catalog says `{}`", case.id, error.code(), case.code);
        assert_eq!(
            error.diagnostic().severity().to_string(),
            case.severity,
            "`{}` reported `{}`, the catalog says `{}`",
            case.id,
            error.diagnostic().severity(),
            case.severity
        );

        let span = error.span().unwrap_or_else(|| panic!("`{}` must locate the rejection", case.id));
        let start = body_start + span.start;
        let end = body_start + span.end;
        assert_eq!(
            &document[start..end],
            case.span_text,
            "`{}` must report the authored bytes [{start}..{end}] (fence body at {body_start})",
            case.id
        );
        comparisons += 1;
    }
    assert_eq!(comparisons, cases.len(), "every diagram case must be compared");
}
