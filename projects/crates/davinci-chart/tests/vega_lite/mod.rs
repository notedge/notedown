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

#[test]
fn constant_mark_color_and_opacity_reach_the_existing_style_channel() {
    let document = r##"{
      "data": {"values": [{"c": "A", "v": 10}, {"c": "B", "v": 20}]},
      "mark": {"type": "bar", "color": "#36c", "opacity": 0.6},
      "encoding": {
        "x": {"field": "c", "type": "nominal"},
        "y": {"field": "v", "type": "quantitative"}
      }
    }"##;
    let chart = vega_lite::lower(document).expect("constant mark styling must lower");

    assert_eq!(chart.spec.mark_color, Some(davinci_scene::Color::rgb(0x33, 0x66, 0xcc)));
    assert_eq!(chart.spec.mark_opacity, Some(0.6));
    for instance in &chart.plan.marks {
        assert_eq!(instance.style.fill, Some(davinci_scene::Color::rgb(0x33, 0x66, 0xcc)), "the constant color fills every bar");
        assert_eq!(instance.style.opacity, 0.6, "the constant opacity reaches `MarkStyle`");
    }

    let svg = chart.to_svg().expect("the styled scene must serialize");
    assert!(svg.contains("fill=\"#3366cc\""), "the authored color must be in the SVG: {svg}");
    assert!(svg.contains("opacity=\"0.6\""), "the authored opacity must be in the SVG: {svg}");
}

#[test]
fn an_encoding_color_channel_overrides_the_constant_mark_color() {
    let document = r##"{
      "data": {"values": [{"c": "A", "v": 10, "k": "red"}, {"c": "B", "v": 20, "k": "blue"}]},
      "mark": {"type": "bar", "color": "#3366cc"},
      "encoding": {
        "x": {"field": "c", "type": "nominal"},
        "y": {"field": "v", "type": "quantitative"},
        "color": {"field": "k", "type": "nominal"}
      }
    }"##;
    let chart = vega_lite::lower(document).expect("the color channel must win");
    let fills: Vec<_> = chart.plan.marks.iter().filter_map(|instance| instance.style.fill).collect();

    assert_eq!(fills.len(), 2);
    assert_ne!(fills[0], davinci_scene::Color::rgb(0x33, 0x66, 0xcc), "the channel color replaces the constant color");
    assert_ne!(fills[0], fills[1], "each category keeps its own color");
}

#[test]
fn log_and_sqrt_scales_lower_through_the_shared_transformed_path() {
    let document = |scale: &str| {
        format!(
            r#"{{"data":{{"values":[{{"c":"A","v":1}},{{"c":"B","v":10}},{{"c":"C","v":100}}]}},
              "mark":"bar",
              "encoding":{{
                "x":{{"field":"c","type":"nominal"}},
                "y":{{"field":"v","type":"quantitative","scale":{{"type":"{scale}"}}}}
              }}}}"#
        )
    };
    let linear = vega_lite::lower(&document("linear")).expect("linear chart");
    let log = vega_lite::lower(&document("log")).expect("log chart");
    let sqrt = vega_lite::lower(&document("sqrt")).expect("sqrt chart");

    let middle_y = |chart: &vega_lite::VegaChart| match &chart.plan.marks[1].geometry {
        davinci_chart::MarkGeometry::Rect(rect) => rect.y,
        _ => panic!("a bar must compile to a rectangle"),
    };
    assert!(middle_y(&linear) > middle_y(&log), "log places the middle decade higher than linear");
    assert!(middle_y(&linear) > middle_y(&sqrt), "sqrt places the middle value higher than linear");
    assert_ne!(middle_y(&log), middle_y(&sqrt));

    let y_axis = log.plan.axes.iter().find(|axis| axis.channel == davinci_chart::Channel::Y).expect("log y axis");
    let labels: Vec<&str> = y_axis.ticks.iter().map(|tick| tick.label.as_str()).collect();
    assert!(labels.contains(&"10") && labels.contains(&"100"), "log tick labels: {labels:?}");
    assert!(log.to_svg().expect("log SVG").contains(">10</text>"));

    let zero = r#"{"data":{"values":[{"c":"A","v":0}]},"mark":"bar","encoding":{"x":{"field":"c","type":"nominal"},"y":{"field":"v","type":"quantitative","scale":{"type":"log"}}}}"#;
    let error = vega_lite::lower(zero).expect_err("zero cannot enter a log domain");
    assert_eq!(error.code(), codes::INVALID_SCALE);
    assert!(error.message().contains("strictly positive"), "{}", error.message());

    let negative_sqrt = r#"{"data":{"values":[{"c":"A","v":-1}]},"mark":"bar","encoding":{"x":{"field":"c","type":"nominal"},"y":{"field":"v","type":"quantitative","scale":{"type":"sqrt"}}}}"#;
    let error = vega_lite::lower(negative_sqrt).expect_err("negative values cannot enter a sqrt domain");
    assert_eq!(error.code(), codes::INVALID_SCALE);
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
        r##"{"data":{"values":[{"c":"A","v":1}]},"mark":{"type":"bar","tooltip":true},"encoding":{"x":{"field":"c","type":"nominal"},"y":{"field":"v","type":"quantitative"}}}"##,
        codes::UNKNOWN_KEY,
        "$.mark.tooltip",
    ),
    (
        r##"{"data":{"values":[{"c":"A","v":1}]},"mark":{"type":"bar","color":"steelblue"},"encoding":{"x":{"field":"c","type":"nominal"},"y":{"field":"v","type":"quantitative"}}}"##,
        codes::UNSUPPORTED_ENCODING,
        "$.mark.color",
    ),
    (
        r##"{"data":{"values":[{"c":"A","v":1}]},"mark":{"type":"bar","opacity":1.4},"encoding":{"x":{"field":"c","type":"nominal"},"y":{"field":"v","type":"quantitative"}}}"##,
        codes::INVALID_SPEC,
        "$.mark.opacity",
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
