//! Vega-Lite subset acceptance: strict lowering into the chart kernel and
//! structured refusals with JSON paths.

mod chrome_gate;
mod matrix;

use davinci_chart::{codes, vega_lite};

const BAR_DOCUMENT: &str = r#"{
  "$schema": "https://vega.github.io/schema/vega-lite/v5.json",
  "title": "Revenue",
  "width": 480,
  "height": 300,
  "data": {
    "values": [
      {"region": "North", "revenue": 3},
      {"region": "South", "revenue": 7},
      {"region": "West", "revenue": 5}
    ]
  },
  "mark": "bar",
  "encoding": {
    "x": {"field": "region", "type": "nominal", "scale": {"type": "band"}},
    "y": {"field": "revenue", "type": "quantitative"}
  }
}"#;

#[test]
fn a_bar_document_lowers_into_a_compiled_plan() {
    let chart = vega_lite::lower(BAR_DOCUMENT).expect("the subset must accept this document");

    assert_eq!(chart.title.as_deref(), Some("Revenue"));
    assert_eq!(chart.options.width, 480.0);
    assert_eq!(chart.options.height, 300.0);
    assert_eq!(chart.plan.canvas.width, 480.0, "the declared width reaches the plan");
    assert_eq!(chart.plan.canvas.height, 300.0, "the declared height reaches the plan");
    assert_eq!(chart.plan.marks.len(), 3, "one bar per observation");

    let svg = chart.to_svg().expect("the scene must serialize to SVG");
    assert!(svg.contains("<svg"), "{svg}");
    assert!(svg.contains("North"), "the category label must be drawn: {svg}");
    assert!(svg.contains("7"), "the value label must be drawn: {svg}");
}

#[test]
fn the_same_document_renders_byte_identical_output_twice() {
    let first = vega_lite::lower(BAR_DOCUMENT).expect("first lowering").to_svg().expect("first svg");
    let second = vega_lite::lower(BAR_DOCUMENT).expect("second lowering").to_svg().expect("second svg");
    assert_eq!(first, second, "lowering must be deterministic");
}

#[test]
fn inline_values_and_aggregates_survive_lowering() {
    let document = r#"{
      "data": {"values": [
        {"team": "A", "amount": 2},
        {"team": "A", "amount": 4},
        {"team": "B", "amount": 6}
      ]},
      "mark": "bar",
      "encoding": {
        "x": {"field": "team", "type": "nominal"},
        "y": {"field": "amount", "type": "quantitative", "aggregate": "sum"}
      }
    }"#;
    let chart = vega_lite::lower(document).expect("the aggregate subset must lower");
    assert_eq!(chart.spec.encoding.y.expect("y binding").aggregate, Some(davinci_chart::Aggregate::Sum));
    assert_eq!(chart.data.row_count(), 3);
}

const REJECTIONS: &[(&str, &str, &str)] = &[
    (
        r#"{"data":{"values":[{"c":"A","v":1}]},"mark":"bar","encoding":{"x":{"field":"c","type":"nominal"},"y":{"field":"v","type":"quantitative"}},"transform":[{"filter":"datum.v > 0"}]}"#,
        codes::UNSUPPORTED_TRANSFORM,
        "$.transform",
    ),
    (
        r#"{"data":{"values":[{"c":"A","v":1}]},"mark":"bar","encoding":{"x":{"field":"c","type":"nominal"},"y":{"field":"v","type":"quantitative"}},"layer":[]}"#,
        codes::UNSUPPORTED_VIEW,
        "$.layer",
    ),
    (
        r#"{"data":{"values":[{"c":"A","v":1}]},"mark":"bar","encoding":{"x":{"field":"c","type":"nominal"},"y":{"field":"v","type":"quantitative"}},"params":[{"name":"brush"}]}"#,
        codes::UNSUPPORTED_INTERACTION,
        "$.params",
    ),
    (
        r#"{"data":{"url":"https://example.com/data.json"},"mark":"bar","encoding":{"x":{"field":"c","type":"nominal"},"y":{"field":"v","type":"quantitative"}}}"#,
        codes::REMOTE_DATA,
        "$.data.url",
    ),
    (
        r#"{"data":{"values":[{"c":"A","v":1}]},"mark":"rule","encoding":{"x":{"field":"c","type":"nominal"},"y":{"field":"v","type":"quantitative"}}}"#,
        codes::UNSUPPORTED_MARK,
        "$.mark",
    ),
    (
        r#"{"data":{"values":[{"c":"A","v":1,"k":"x"}]},"mark":"point","encoding":{"x":{"field":"c","type":"nominal"},"y":{"field":"v","type":"quantitative"},"shape":{"field":"k","type":"nominal"}}}"#,
        codes::UNSUPPORTED_CHANNEL,
        "$.encoding.shape",
    ),
    (
        r#"{"data":{"values":[{"c":"A","v":1}]},"mark":"bar","encoding":{"x":{"field":"c","type":"nominal","bin":true},"y":{"field":"v","type":"quantitative"}}}"#,
        codes::UNSUPPORTED_ENCODING,
        "$.encoding.x.bin",
    ),
    (
        r#"{"data":{"values":[{"c":"A","v":1}]},"mark":"line","encoding":{"x":{"field":"c","type":"nominal"},"y":{"field":"v","type":"quantitative","scale":{"type":"log"}}}}"#,
        codes::UNSUPPORTED_SCALE,
        "$.encoding.y.scale.type",
    ),
    (
        r#"{"data":{"values":[{"c":"A","v":1}]},"mark":"line","encoding":{"x":{"field":"c","type":"nominal"},"y":{"field":"v","type":"quantitative","scale":{"type":"linear","zero":true}}}}"#,
        codes::UNSUPPORTED_SCALE,
        "$.encoding.y.scale.zero",
    ),
    (
        r#"{"data":{"values":[{"c":"A","v":1}]},"mark":"bar","encoding":{"x":{"field":"c","type":"nominal"},"y":{"field":"v","type":"quantitative"}},"foo":true}"#,
        codes::UNKNOWN_KEY,
        "$.foo",
    ),
    (
        r#"{"data":{"values":[{"c":"A","v":1}]},"mark":"bar","encoding":{"x":{"field":"c","type":"nominal","axis":{}},"y":{"field":"v","type":"quantitative"}}}"#,
        codes::UNSUPPORTED_ENCODING,
        "$.encoding.x.axis",
    ),
    (
        r##"{"data":{"values":[{"c":"A","v":1}]},"mark":{"type":"bar","color":"#3366cc"},"encoding":{"x":{"field":"c","type":"nominal"},"y":{"field":"v","type":"quantitative"}}}"##,
        codes::UNSUPPORTED_ENCODING,
        "$.mark.color",
    ),
    (
        r#"{"data":{"values":[{"c":"A","v":1}]},"mark":"bar","encoding":{"x":{"field":"c","type":"nominal"},"y":{"field":"v","type":"quantitative","aggregate":"median"}}}"#,
        codes::UNSUPPORTED_ENCODING,
        "$.encoding.y.aggregate",
    ),
    (
        r#"{"data":{"values":[{"when":"2020-01-01","v":1}]},"mark":"line","encoding":{"x":{"field":"when","type":"temporal"},"y":{"field":"v","type":"quantitative"}}}"#,
        codes::TYPE_MISMATCH,
        "$.data.values[0].when",
    ),
    ("{ not json", codes::INVALID_JSON, "$"),
    ("[]", codes::INVALID_SPEC, "$"),
];

#[test]
fn every_rejection_is_structured_and_named_with_a_json_path() {
    for (document, expected_code, expected_path) in REJECTIONS {
        let error = vega_lite::lower(document).expect_err("this document must be refused");
        assert_eq!(error.code(), *expected_code, "document `{document}` produced `{}`", error.message());
        assert!(error.message().contains(expected_path), "document `{document}` must name `{expected_path}`, got `{}`", error.message());
    }
}
