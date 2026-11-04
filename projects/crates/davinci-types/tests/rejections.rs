//! The authoritative rejection catalog must stay exhaustive and self-consistent.

use std::collections::BTreeSet;

use davinci_types::{CATALOG_JSON, cases, code_docs, codes, matrix};

const SEVERITIES: [&str; 2] = ["error", "warning"];
const MATRIX_STATUSES: [&str; 3] = ["supported", "approximated", "rejected"];
const MATRIX_AREAS: [&str; 2] = ["chart", "diagram"];

/// The published matrix totals. A row added or reclassified must update this
/// gate in the same change, so the doc, the catalog and the tests stay one
/// source of truth.
const MATRIX_TOTAL: usize = 83;
const MATRIX_SUPPORTED: usize = 33;
const MATRIX_APPROXIMATED: usize = 9;
const MATRIX_REJECTED: usize = 41;

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
fn every_rejection_matrix_row_points_at_a_fixture_or_a_named_reason() {
    let rows = matrix();
    assert_eq!(rows.len(), MATRIX_TOTAL, "the catalog matrix must mirror the published matrix");
    for status in MATRIX_STATUSES {
        let expected = match status {
            "supported" => MATRIX_SUPPORTED,
            "approximated" => MATRIX_APPROXIMATED,
            _ => MATRIX_REJECTED,
        };
        let actual = rows.iter().filter(|row| row.status == status).count();
        assert_eq!(actual, expected, "the matrix must carry {expected} `{status}` rows, got {actual}");
    }

    let docs = code_docs();
    let documented: BTreeSet<&str> = docs.iter().map(|doc| doc.code.as_str()).collect();
    let shipped = cases();
    let case_ids: BTreeSet<&str> = shipped.iter().map(|case| case.id.as_str()).collect();
    let mut ids = BTreeSet::new();
    let mut referenced = BTreeSet::new();

    for row in &rows {
        assert!(ids.insert(row.id.clone()), "duplicate matrix row id `{}`", row.id);
        assert!(MATRIX_AREAS.contains(&row.area.as_str()), "`{}` declares an unknown area `{}`", row.id, row.area);
        assert!(MATRIX_STATUSES.contains(&row.status.as_str()), "`{}` declares an unknown status `{}`", row.id, row.status);
        assert!(!row.item.trim().is_empty(), "`{}` has no description", row.id);

        if row.status == "supported" {
            assert!(row.code.is_none() && row.reason.is_none(), "`{}` is supported but names a code or a reason", row.id);
            for case in &row.cases {
                assert!(case_ids.contains(case.as_str()), "`{}` references `{case}`, which the catalog does not carry", row.id);
                referenced.insert(case.as_str());
            }
            continue;
        }

        // A degraded row names the code it reports, and either the fixture that
        // proves it or the reason the fixture cannot exist. A supported row owns
        // no diagnostic, so it may carry neither a code nor a reason.
        if let Some(code) = row.code.as_deref() {
            assert!(documented.contains(code), "`{}` names `{code}`, which the catalog does not document", row.id);
        }
        else {
            assert_ne!(row.status, "rejected", "`{}` is rejected but names no code", row.id);
        }
        for case in &row.cases {
            assert!(case_ids.contains(case.as_str()), "`{}` references `{case}`, which the catalog does not carry", row.id);
            referenced.insert(case.as_str());
        }
        if row.status == "rejected" {
            assert!(
                !row.cases.is_empty() || row.reason.as_deref().is_some_and(|reason| !reason.trim().is_empty()),
                "`{}` is rejected but points at neither a fixture nor a reason",
                row.id
            );
        }
    }

    for case in &case_ids {
        assert!(referenced.contains(case), "catalog case `{case}` is not reachable from any rejection row");
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
