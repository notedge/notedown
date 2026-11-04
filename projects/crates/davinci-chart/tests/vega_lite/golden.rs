//! Byte-level audit for the default `zero` and `nice` scale semantics.
//!
//! Every case renders an untitled chart so the bytes only track the scale
//! defaults. A changed golden must be accepted together with a written note in
//! `tests/fixtures/chart-golden/audit.json`, so a silent domain shift cannot
//! land as a test update.

use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

use davinci_chart::vega_lite;

const AUDIT: &str = include_str!("../fixtures/chart-golden/audit.json");

/// One audited document. The id names both the golden file and the audit entry.
const CASES: &[(&str, &str)] = &[
    (
        "bar-zero-default",
        r#"{"width":320,"height":200,"data":{"values":[{"r":"A","v":3},{"r":"B","v":7}]},"mark":"bar","encoding":{"x":{"field":"r","type":"nominal"},"y":{"field":"v","type":"quantitative"}}}"#,
    ),
    (
        "bar-zero-off",
        r#"{"width":320,"height":200,"data":{"values":[{"r":"A","v":3},{"r":"B","v":7}]},"mark":"bar","encoding":{"x":{"field":"r","type":"nominal"},"y":{"field":"v","type":"quantitative","scale":{"type":"linear","zero":false}}}}"#,
    ),
    (
        "bar-nice-on",
        r#"{"width":320,"height":200,"data":{"values":[{"r":"A","v":3},{"r":"B","v":7}]},"mark":"bar","encoding":{"x":{"field":"r","type":"nominal"},"y":{"field":"v","type":"quantitative","scale":{"type":"linear","domain":[0,17],"nice":true}}}}"#,
    ),
    (
        "bar-nice-off",
        r#"{"width":320,"height":200,"data":{"values":[{"r":"A","v":3},{"r":"B","v":7}]},"mark":"bar","encoding":{"x":{"field":"r","type":"nominal"},"y":{"field":"v","type":"quantitative","scale":{"type":"linear","domain":[0,17],"nice":false}}}}"#,
    ),
    (
        "bar-zero-off-nice-on",
        r#"{"width":320,"height":200,"data":{"values":[{"r":"A","v":3},{"r":"B","v":7}]},"mark":"bar","encoding":{"x":{"field":"r","type":"nominal"},"y":{"field":"v","type":"quantitative","scale":{"type":"linear","zero":false,"nice":true}}}}"#,
    ),
    (
        "point-log",
        r#"{"width":320,"height":200,"data":{"values":[{"r":"A","v":3},{"r":"B","v":7}]},"mark":"point","encoding":{"x":{"field":"r","type":"nominal"},"y":{"field":"v","type":"quantitative","scale":{"type":"log"}}}}"#,
    ),
];

fn svg_for(document: &str) -> String {
    vega_lite::lower(document).expect("golden documents must lower").to_svg().expect("golden scenes must resolve")
}

#[test]
fn golden_chart_bytes_change_only_with_a_recorded_audit_note() {
    let audit: serde_json::Value = serde_json::from_str(AUDIT).expect("the golden audit is valid JSON");
    let entries = audit["cases"].as_array().expect("the audit carries a `cases` array");
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/chart-golden");

    let mut audited = BTreeSet::new();
    for entry in entries {
        let id = entry["id"].as_str().expect("every audit entry names an id");
        let note = entry["note"].as_str().unwrap_or_default();
        assert!(!note.trim().is_empty(), "audit entry `{id}` must record why its bytes are the accepted ones");
        assert!(audited.insert(id.to_string()), "duplicate audit entry `{id}`");
    }

    for (id, document) in CASES {
        assert!(audited.contains(*id), "golden case `{id}` has no audit entry, so a changed domain would land silently");
        let actual = svg_for(document);
        let path = dir.join(format!("{id}.svg"));
        if std::env::var_os("UPDATE_GOLDEN").is_some() {
            fs::write(&path, &actual).expect("write the golden svg");
            continue;
        }
        let expected = fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("golden `{id}` is unreadable ({error}): regenerate it with `UPDATE_GOLDEN=1` and review the bytes"));
        assert_eq!(
            actual, expected,
            "golden `{id}` changed: update `tests/fixtures/chart-golden/{id}.svg` and write the reason in `audit.json` in the same change"
        );
    }

    assert_eq!(audited.len(), CASES.len(), "`audit.json` carries entries without a golden case");
}
