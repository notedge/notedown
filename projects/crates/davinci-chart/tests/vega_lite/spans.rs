//! Every refusal names its JSON path and the authored bytes behind it.

use davinci_chart::{codes, vega_lite};

#[test]
fn a_rejection_carries_the_exact_value_range() {
    let document = r#"{"data":{"values":[{"c":"A","v":1}]},"mark":{"type":"bar","color":"steelblue"},"encoding":{"x":{"field":"c","type":"nominal"},"y":{"field":"v","type":"quantitative"}}}"#;

    let error = vega_lite::lower(document).expect_err("named colors stay outside the subset");
    assert_eq!(error.code(), codes::UNSUPPORTED_ENCODING);
    assert_eq!(error.path(), Some("$.mark.color"));

    let span = error.span().expect("the offending value must have a byte range");
    assert_eq!(&document[span], "\"steelblue\"", "the range must cover exactly the rejected value");
}

#[test]
fn nested_array_entries_resolve_to_their_own_cell() {
    let document = r#"{"data":{"values":[{"when":1,"v":1},{"when":"2020-01-01","v":2}]},"mark":"line","encoding":{"x":{"field":"when","type":"temporal"},"y":{"field":"v","type":"quantitative"}}}"#;

    let error = vega_lite::lower(document).expect_err("a string temporal cell is a type mismatch");
    assert_eq!(error.code(), codes::TYPE_MISMATCH);
    assert_eq!(error.path(), Some("$.data.values[1].when"));

    let span = error.span().expect("the offending cell must have a byte range");
    assert_eq!(&document[span], "\"2020-01-01\"");
}

#[test]
fn unsupported_constructs_point_at_the_construct_not_the_document() {
    for (document, path) in [
        (r#"{"data":{"values":[]},"mark":"bar","encoding":{"x":{"field":"c","type":"nominal"}},"transform":[]}"#, "$.transform"),
        (r#"{"data":{"values":[]},"mark":"bar","encoding":{"x":{"field":"c","type":"nominal"}},"layer":[]}"#, "$.layer"),
        (r#"{"data":{"values":[]},"mark":"bar","encoding":{"x":{"field":"c","type":"nominal"}},"params":[]}"#, "$.params"),
        (r#"{"data":{"url":"https://example.com"},"mark":"bar","encoding":{"x":{"field":"c","type":"nominal"}}}"#, "$.data.url"),
    ] {
        let error = vega_lite::lower(document).expect_err("this document must be refused");
        assert_eq!(error.path(), Some(path), "{}", error.message());

        let span = error.span().expect("a refusal keeps a range");
        assert!(span.start < span.end, "the range must be non-empty: {span:?}");
        assert!(span.end <= document.len());
    }
}

#[test]
fn invalid_json_reports_the_position_of_the_failure() {
    let document = "{\"data\": {\"values\": []}, \"mark\": }";
    let error = vega_lite::lower(document).expect_err("the document is malformed");
    assert_eq!(error.code(), codes::INVALID_JSON);

    let span = error.span().expect("even a parse failure keeps a range");
    assert!(span.end <= document.len());
    assert_eq!(&document[span.start..span.end], "}");
}

#[test]
fn every_document_level_failure_still_has_a_range() {
    let document = "[]";
    let error = vega_lite::lower(document).expect_err("a top-level array is invalid");
    assert_eq!(error.code(), codes::INVALID_SPEC);
    assert_eq!(error.span(), Some(0..2));
    assert_eq!(&document[error.span().unwrap()], "[]");
}
