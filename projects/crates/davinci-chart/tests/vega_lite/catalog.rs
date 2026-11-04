//! The chart entry consumes the shared rejection catalog.
//!
//! Every expected code, severity, and span comes from
//! `davinci_types::rejections`, so this test owns no private expectations.

use std::collections::BTreeSet;

use davinci_chart::{codes, vega_lite};
use davinci_types::{cases_for, code_docs};

/// Every chart code this crate can emit.
const SHIPPED: [&str; 22] = [
    codes::EMPTY_DATA,
    codes::RAGGED_COLUMN,
    codes::TYPE_MISMATCH,
    codes::MISSING_VALUE,
    codes::NON_FINITE_VALUE,
    codes::DUPLICATE_FIELD,
    codes::UNKNOWN_FIELD,
    codes::INVALID_SPEC,
    codes::INVALID_SCALE,
    codes::EMPTY_DOMAIN,
    codes::DEGENERATE_DOMAIN,
    codes::APPROXIMATED,
    codes::INVALID_JSON,
    codes::UNKNOWN_KEY,
    codes::UNSUPPORTED_MARK,
    codes::UNSUPPORTED_CHANNEL,
    codes::UNSUPPORTED_ENCODING,
    codes::UNSUPPORTED_SCALE,
    codes::UNSUPPORTED_TRANSFORM,
    codes::UNSUPPORTED_VIEW,
    codes::UNSUPPORTED_INTERACTION,
    codes::REMOTE_DATA,
];

#[test]
fn the_catalog_documents_every_chart_code_and_invents_none() {
    let docs = code_docs();
    let documented: BTreeSet<&str> = docs
        .iter()
        .map(|doc| doc.code.as_str())
        .filter(|code| code.starts_with("davinci.chart."))
        .collect();

    for code in SHIPPED {
        assert!(documented.contains(code), "`{code}` is emitted but missing from the catalog");
    }
    for code in &documented {
        assert!(SHIPPED.contains(code), "the catalog documents `{code}`, which this crate cannot emit");
    }
}

#[test]
fn the_chart_entry_reports_the_catalog_code_severity_and_span() {
    let cases = cases_for("vega-lite");
    assert!(!cases.is_empty(), "the catalog must carry chart cases");

    let mut comparisons = 0;
    for case in &cases {
        // Author the fence exactly as a host would, then keep the byte offset so
        // the reported span is compared in authored bytes, not in a normalized
        // copy of the source.
        let document = format!("# Rejection\n\n```vega-lite\n{}\n```\n", case.source);
        let body_start = document.find(&case.source).unwrap_or_else(|| panic!("`{}` must sit in the fence", case.id));

        let error = match vega_lite::lower(&case.source) {
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
    assert_eq!(comparisons, cases.len(), "every chart case must be compared");
}
