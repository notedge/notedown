//! Vega-Lite subset acceptance: strict lowering into the chart kernel and
//! structured refusals with JSON paths.

mod chrome_gate;
mod matrix;
mod spans;

use davinci_chart::{Channel, Scale, codes, vega_lite};

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

#[test]
fn page_hooks_namespace_every_chart_id_and_keep_mark_colors() {
    let document = r##"{"data":{"values":[{"c":"A","v":10},{"c":"B","v":20}]},"mark":{"type":"bar","color":"#3366cc"},"encoding":{"x":{"field":"c","type":"nominal"},"y":{"field":"v","type":"quantitative"}}}"##;
    let first = vega_lite::lower(document).expect("first chart");
    let second = vega_lite::lower(document).expect("second chart");

    let plain = first.to_svg().expect("plain svg");
    assert!(plain.contains("id=\"chart-bar-0\""), "the plain path keeps bare ids");
    assert!(!plain.contains("class=\""), "the plain path emits no role classes");

    let left = first.to_svg_with_hooks("chart-0", "chart").expect("first hooked svg");
    let right = second.to_svg_with_hooks("chart-1", "chart").expect("second hooked svg");

    let mut ids = element_ids(&left);
    ids.extend(element_ids(&right));
    ids.sort();
    let mut deduped = ids.clone();
    deduped.dedup();
    assert_eq!(ids, deduped, "every id on the page must be unique: {ids:?}");
    assert_eq!(ids.iter().filter(|id| id.starts_with("chart-0-")).count() + ids.iter().filter(|id| id.starts_with("chart-1-")).count(), ids.len());
    assert!(ids.iter().all(|id| id.starts_with("chart-0-") || id.starts_with("chart-1-")));

    assert!(left.contains("class=\"chart\"") && right.contains("class=\"chart\""));
    for role in ["chart-axis", "chart-tick", "chart-mark"] {
        assert!(
            left.contains(&format!("class=\"{role}\"")),
            "the `{role}` class must reach the element: {left}"
        );
    }
    assert!(left.contains("style=\"fill: #3366cc;stroke: none\""), "the authored mark color must survive theming: {left}");

    assert!(left.contains(".chart .chart-axis line { stroke:"), "the palette must own axis chrome: {left}");
    assert!(left.contains(".chart .chart-tick text { fill:"), "the palette must own tick labels");
    assert!(left.contains(".chart .chart-mark { fill: none; stroke: none }"), "the palette must declare a neutral mark default");
}

fn element_ids(svg: &str) -> Vec<String> {
    svg.match_indices("id=\"")
        .map(|(index, _)| {
            let rest = &svg[index + 4..];
            rest[..rest.find('"').expect("a closed id attribute")].to_string()
        })
        .collect()
}

#[test]
fn zero_and_nice_are_opt_in_scale_options() {
    let line = r#"{"data":{"values":[{"c":"A","v":10},{"c":"B","v":20}]},"mark":"line","encoding":{"x":{"field":"c","type":"nominal"},"y":{"field":"v","type":"quantitative"}}}"#;
    let line_explicit_defaults = r#"{"data":{"values":[{"c":"A","v":10},{"c":"B","v":20}]},"mark":"line","encoding":{"x":{"field":"c","type":"nominal"},"y":{"field":"v","type":"quantitative","scale":{"type":"linear","zero":false,"nice":false}}}}"#;
    let bar = r#"{"data":{"values":[{"c":"A","v":10},{"c":"B","v":20}]},"mark":"bar","encoding":{"x":{"field":"c","type":"nominal"},"y":{"field":"v","type":"quantitative"}}}"#;
    let clipped_bar = r#"{"data":{"values":[{"c":"A","v":10},{"c":"B","v":20}]},"mark":"bar","encoding":{"x":{"field":"c","type":"nominal"},"y":{"field":"v","type":"quantitative","scale":{"type":"linear","zero":false}}}}"#;
    let niced = r#"{"data":{"values":[{"c":"A","v":3},{"c":"B","v":97}]},"mark":"bar","encoding":{"x":{"field":"c","type":"nominal"},"y":{"field":"v","type":"quantitative","scale":{"type":"linear","nice":true}}}}"#;

    let line_svg = vega_lite::lower(line).expect("line chart").to_svg().expect("line svg");
    let explicit_svg = vega_lite::lower(line_explicit_defaults).expect("explicit line chart").to_svg().expect("explicit svg");
    assert_eq!(line_svg, explicit_svg, "on a line chart, explicit `zero: false` and `nice: false` stay byte-identical to the default");

    let chart = vega_lite::lower(bar).expect("bar chart");
    let y_scale = chart.plan.scale(Channel::Y).and_then(Scale::linear).expect("bar y scale");
    assert_eq!(y_scale.domain, (0.0, 20.0), "a bar anchors its y domain at the origin by default");
    let svg = chart.to_svg().expect("bar svg");
    assert!(svg.contains(">0</text>"), "the zero tick label must be drawn: {svg}");
    assert!(svg.contains("height=\"144\""), "the 10 bar keeps half the plot height instead of collapsing: {svg}");

    let clipped = vega_lite::lower(clipped_bar).expect("clipped bar chart");
    let clipped_y = clipped.plan.scale(Channel::Y).and_then(Scale::linear).expect("clipped y scale");
    assert_eq!(clipped_y.domain, (10.0, 20.0), "`zero: false` restores the observed extremes");
    assert!(clipped.to_svg().expect("clipped svg").contains("height=\"0\""), "opting out of zero clips the minimum bar");

    let nice = vega_lite::lower(niced).expect("nice chart");
    let nice_y = nice.plan.scale(Channel::Y).and_then(Scale::linear).expect("nice y scale");
    assert_eq!(nice_y.domain, (0.0, 100.0), "`nice` rounds 3..97 outward to 0..100");
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
        r#"{"data":{"values":[{"c":"A","v":1}]},"mark":"bar","encoding":{"x":{"field":"c","type":"nominal"},"y":{"field":"v","type":"quantitative","scale":{"type":"log","zero":true}}}}"#,
        codes::UNSUPPORTED_SCALE,
        "$.encoding.y.scale.zero",
    ),
    (
        r#"{"data":{"values":[{"c":"A","v":1}]},"mark":"bar","encoding":{"x":{"field":"c","type":"nominal"},"y":{"field":"v","type":"quantitative","scale":{"type":"linear","domain":[0,10],"zero":true}}}}"#,
        codes::INVALID_SCALE,
        "$.encoding.y.scale.zero",
    ),
    (
        r#"{"data":{"values":[{"c":"A","v":1}]},"mark":"bar","encoding":{"x":{"field":"c","type":"nominal"},"y":{"field":"v","type":"quantitative","scale":{"type":"linear","nice":"yes"}}}}"#,
        codes::INVALID_SPEC,
        "$.encoding.y.scale.nice",
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
