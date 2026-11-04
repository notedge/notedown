//! The authoritative rejection catalog must stay exhaustive and self-consistent.

use std::collections::BTreeSet;

use davinci_types::{CATALOG_JSON, cases, code_docs, codes};

const SEVERITIES: [&str; 2] = ["error", "warning"];

/// Every code the subsystem can emit. A new constant must land here and in the
/// catalog in the same change, otherwise the catalog stops being authoritative.
fn shipped_codes() -> Vec<&'static str> {
    vec![
        codes::INVALID_GEOMETRY,
        codes::INVALID_STYLE,
        codes::INVALID_TRANSFORM,
        codes::SERIALIZATION_FAILED,
        codes::PARSE_FAILED,
        codes::AST_UNAVAILABLE,
        codes::UNSUPPORTED,
        codes::APPROXIMATED,
        codes::EMPTY_DIAGRAM,
    ]
}

#[test]
fn the_catalog_json_parses_and_round_trips() {
    let docs = code_docs();
    let parsed: serde_json::Value = serde_json::from_str(CATALOG_JSON).expect("the catalog is JSON");
    assert_eq!(parsed["codes"].as_array().expect("codes array").len(), docs.len());
    assert!(!docs.is_empty(), "the catalog documents every code");
    assert!(!cases().is_empty(), "the catalog carries rejection cases");
}

#[test]
fn every_documented_code_is_a_real_constant_and_every_constant_is_documented() {
    let docs = code_docs();
    let documented: BTreeSet<&str> = docs.iter().map(|doc| doc.code.as_str()).collect();
    for code in shipped_codes() {
        assert!(documented.contains(code), "`{code}` is emitted but missing from the catalog");
    }
    // Chart codes belong to `davinci-chart`, which asserts its own coverage.
    let local: BTreeSet<&str> = documented.iter().copied().filter(|code| !code.starts_with("davinci.chart.")).collect();
    for code in &local {
        assert!(shipped_codes().contains(code), "the catalog documents `{code}`, which no constant emits");
    }
    assert_eq!(local.len(), shipped_codes().len(), "the scene and diagram codes must all be documented");
    for code in &documented {
        assert!(
            ["davinci.scene.", "davinci.diagram.", "davinci.chart."].iter().any(|prefix| code.starts_with(prefix)),
            "`{code}` is outside the three DaVinci components"
        );
    }
}

#[test]
fn every_code_documents_a_meaning_a_trigger_and_a_span() {
    for doc in code_docs() {
        assert!(SEVERITIES.contains(&doc.severity.as_str()), "`{}` has severity `{}`", doc.code, doc.severity);
        assert!(!doc.meaning.trim().is_empty(), "`{}` has no author-facing meaning", doc.code);
        assert!(!doc.trigger.trim().is_empty(), "`{}` has no trigger", doc.code);
        assert!(!doc.span.trim().is_empty(), "`{}` has no span description", doc.code);
    }
}

#[test]
fn every_case_points_at_an_exact_authored_slice() {
    for case in cases() {
        assert!(
            ["vega-lite", "mermaid", "dot", "plantuml"].contains(&case.language.as_str()),
            "`{}` declares an unknown language `{}`",
            case.id,
            case.language
        );
        assert!(SEVERITIES.contains(&case.severity.as_str()), "`{}` has severity `{}`", case.id, case.severity);
        let at = case
            .source
            .find(&case.span_text)
            .unwrap_or_else(|| panic!("`{}`: `span_text` does not occur in the authored source", case.id));
        assert_eq!(&case.source[at..at + case.span_text.len()], case.span_text, "`{}` must be byte-exact", case.id);
    }
}

#[test]
fn case_ids_are_unique_and_the_catalog_covers_both_domains() {
    let mut ids = BTreeSet::new();
    for case in cases() {
        assert!(ids.insert(case.id.clone()), "duplicate case id `{}`", case.id);
    }
    let all = cases();
    let languages: BTreeSet<&str> = all.iter().map(|case| case.language.as_str()).collect();
    assert!(languages.contains("vega-lite"), "the chart domain needs its own cases");
    assert!(
        languages.contains("mermaid") && languages.contains("dot") && languages.contains("plantuml"),
        "every diagram frontend needs a case: {languages:?}"
    );
}
