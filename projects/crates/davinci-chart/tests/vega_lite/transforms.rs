//! `bin` and `timeUnit` are chart transforms, not mark options.
//!
//! A transformed positional channel becomes an ordered categorical domain, so
//! the assertions here read the trained band/point scale and the axis labels
//! produced from the derived buckets rather than any document syntax.

use davinci_chart::{Channel, ChannelTransform, Scale, TimeUnit, Transforms, codes, vega_lite};

fn tick_labels(document: &str, channel: Channel) -> Vec<String> {
    let chart = vega_lite::lower(document).expect("the document is inside the chart subset");
    chart
        .plan
        .axes
        .iter()
        .find(|axis| axis.channel == channel)
        .map(|axis| axis.ticks.iter().map(|tick| tick.label.clone()).collect())
        .unwrap_or_default()
}

fn bar(values: &str, encoding: &str) -> String {
    format!(r#"{{"data":{{"values":[{values}]}},"mark":"bar","encoding":{encoding}}}"#)
}

fn assert_close(actual: f64, expected: f64, what: &str) {
    assert!((actual - expected).abs() < 1e-9, "{what}: expected {expected}, got {actual}");
}

/// The bar rectangles of a plan, in mark order.
fn bar_rects(chart: &davinci_chart::vega_lite::VegaChart) -> Vec<(f64, f64, f64, f64)> {
    chart
        .plan
        .marks
        .iter()
        .map(|mark| match &mark.geometry {
            davinci_chart::MarkGeometry::Rect(rect) => (rect.x, rect.y, rect.width, rect.height),
            other => panic!("a bar must compile to a rectangle, got {other:?}"),
        })
        .collect()
}

fn axis_positions(chart: &davinci_chart::vega_lite::VegaChart, channel: Channel) -> Vec<(String, f64, f64)> {
    chart
        .plan
        .axes
        .iter()
        .find(|axis| axis.channel == channel)
        .map(|axis| axis.ticks.iter().map(|tick| (tick.label.clone(), tick.position.x, tick.position.y)).collect())
        .unwrap_or_default()
}

#[test]
fn bin_derives_half_open_buckets_from_the_data_extent() {
    let document = bar(
        r#"{"v":0},{"v":3},{"v":7},{"v":12}"#,
        r#"{"x":{"field":"v","type":"quantitative","bin":{"step":5}},"y":{"field":"v","type":"quantitative"}}"#,
    );
    assert_eq!(tick_labels(&document, Channel::X), ["0", "5", "10"]);
}

#[test]
fn a_value_on_the_declared_upper_bound_folds_into_the_last_bucket() {
    let document = bar(
        r#"{"v":0},{"v":10}"#,
        r#"{"x":{"field":"v","type":"quantitative","bin":{"step":10,"extent":[0,10]}},"y":{"field":"v","type":"quantitative"}}"#,
    );
    assert_eq!(tick_labels(&document, Channel::X), ["0"]);
}

#[test]
fn a_value_below_the_declared_extent_is_refused_instead_of_clamped() {
    let document = bar(
        r#"{"v":0},{"v":-1}"#,
        r#"{"x":{"field":"v","type":"quantitative","bin":{"step":10,"extent":[0,100]}},"y":{"field":"v","type":"quantitative"}}"#,
    );
    let error = vega_lite::lower(&document).expect_err("a clamped bucket would misreport the datum");
    assert_eq!(error.code(), codes::INVALID_SPEC);
    assert!(error.message().contains("below the declared `bin` extent"), "{}", error.message());
    // The refusal is raised while projecting data rows in the language-neutral
    // kernel, which carries no JSON path, so the documented fallback is the
    // whole document range rather than the offending `-1` cell.
    let span = error.span().expect("every chart refusal carries a byte range");
    assert_eq!(span, 0..document.len(), "the extent refusal falls back to the whole document");
    assert_eq!(&document[span], document.as_str());
}

#[test]
fn declared_extent_keeps_the_lower_bound_and_its_float_neighbour_together() {
    let document = bar(
        r#"{"v":0},{"v":9.999},{"v":10}"#,
        r#"{"x":{"field":"v","type":"quantitative","bin":{"step":10,"extent":[0,30]}},"y":{"field":"v","type":"quantitative"}}"#,
    );
    let chart = vega_lite::lower(&document).expect("the values are inside the declared extent");
    let band = chart.plan.scale(Channel::X).and_then(Scale::band).expect("a `bin` channel trains a band scale");
    assert_eq!(band.domain, ["0", "10"]);
    let sources: Vec<&str> = chart.plan.marks.iter().map(|mark| mark.source.as_str()).collect();
    assert_eq!(sources, ["0", "0", "10"], "`0` and `9.999` share the half-open `[0, 10)` bucket");
}


#[test]
fn nice_aligns_the_extent_outward_to_whole_steps() {
    let document = bar(
        r#"{"v":3},{"v":7},{"v":17}"#,
        r#"{"x":{"field":"v","type":"quantitative","bin":{"step":5,"nice":true}},"y":{"field":"v","type":"quantitative"}}"#,
    );
    assert_eq!(tick_labels(&document, Channel::X), ["0", "5", "15"]);
}

#[test]
fn bin_bars_sit_on_their_buckets_with_heights_measured_from_zero() {
    let document = bar(
        r#"{"v":4,"w":4},{"v":15,"w":8}"#,
        r#"{"x":{"field":"v","type":"quantitative","bin":{"step":10,"nice":true}},"y":{"field":"w","type":"quantitative"}}"#,
    );
    let chart = vega_lite::lower(&document).expect("the document is inside the chart subset");
    let band = chart.plan.scale(Channel::X).and_then(Scale::band).expect("a `bin` channel trains a band scale");
    assert_eq!(band.domain, ["0", "10"]);
    assert_close(band.band_width, 226.28571428571428, "band width");
    assert_close(band.step - band.band_width, 25.142857142857167, "inter-bar gap");

    assert_eq!(chart.plan.plot.x, 56.0);
    assert_eq!(chart.plan.plot.y, 56.0);
    assert_eq!(chart.plan.plot.width, 528.0);
    assert_eq!(chart.plan.plot.height, 288.0);

    let rects = bar_rects(&chart);
    assert_eq!(rects.len(), 2, "one bar per bucket that has a row");
    assert_close(rects[0].0, 81.14285714285714, "first bucket left edge");
    assert_close(rects[0].1, 200.0, "first bar top");
    assert_close(rects[0].2, 226.28571428571428, "first bar width");
    assert_close(rects[0].3, 144.0, "first bar height for value 4 of a `(0, 8)` domain");
    assert_close(rects[1].0, 332.57142857142856, "second bucket left edge");
    assert_close(rects[1].1, 56.0, "second bar top, flush with the plot top");
    assert_close(rects[1].2, 226.28571428571428, "second bar width");
    assert_close(rects[1].3, 288.0, "second bar height for value 8 of a `(0, 8)` domain");
    assert_close(rects[0].3 + rects[0].1, 344.0, "both bars start at the zero baseline");
    assert_close(rects[1].3 + rects[1].1, 344.0, "both bars start at the zero baseline");
    assert_close(rects[1].3 / rects[0].3, 8.0 / 4.0, "bar heights keep the value ratio");

    let ticks = axis_positions(&chart, Channel::X);
    assert_eq!(ticks.len(), 2);
    assert_eq!(ticks[0].0, "0");
    assert_eq!(ticks[1].0, "10");
    assert_close(ticks[0].1, 194.28571428571428, "first tick sits at the bucket centre");
    assert_close(ticks[1].1, 445.7142857142857, "second tick sits at the bucket centre");
    assert_eq!(ticks[0].2, 344.0);
    assert_eq!(ticks[1].2, 344.0);
}

#[test]
fn time_unit_bars_collapse_instants_of_one_calendar_unit_to_one_coordinate() {
    let document = bar(
        r#"{"t":1705276800000,"w":4},{"t":1706745600000,"w":6},{"t":1706918400000,"w":8}"#,
        r#"{"x":{"field":"t","type":"temporal","timeUnit":"yearmonth"},"y":{"field":"w","type":"quantitative"}}"#,
    );
    let chart = vega_lite::lower(&document).expect("the document is inside the chart subset");
    let band = chart.plan.scale(Channel::X).and_then(Scale::band).expect("a `timeUnit` channel trains a band scale");
    assert_eq!(band.domain, ["2024-01", "2024-02"]);

    let rects = bar_rects(&chart);
    assert_eq!(rects.len(), 3, "every row keeps its own mark");
    assert_close(rects[0].0, 81.14285714285714, "January bar left edge");
    assert_close(rects[1].0, 332.57142857142856, "February bar left edge");
    assert_close(rects[2].0, 332.57142857142856, "two February instants share one x coordinate");
    assert_close(rects[0].2, 226.28571428571428, "bars keep the band width");
    assert_close(rects[0].3, 144.0, "January bar height for value 4");
    assert_close(rects[1].3, 216.0, "February bar height for value 6");
    assert_close(rects[2].3, 288.0, "February bar height for value 8");

    let ticks = axis_positions(&chart, Channel::X);
    assert_eq!(ticks.iter().map(|tick| tick.0.as_str()).collect::<Vec<_>>(), ["2024-01", "2024-02"]);
    assert_close(ticks[0].1, 194.28571428571428, "January tick at the band centre");
    assert_close(ticks[1].1, 445.7142857142857, "February tick at the band centre");
}

#[test]
fn time_units_truncate_to_utc_calendar_labels() {
    let document = bar(
        r#"{"t":1705276800000},{"t":1706918400000},{"t":1706990400000}"#,
        r#"{"x":{"field":"t","type":"temporal","timeUnit":"yearmonth"},"y":{"field":"t","type":"temporal"}}"#,
    );
    assert_eq!(tick_labels(&document, Channel::X), ["2024-01", "2024-02"]);
}

#[test]
fn every_advertised_time_unit_is_accepted_with_an_optional_utc_prefix() {
    for unit in TimeUnit::ALL {
        for keyword in [unit.as_str().to_string(), format!("utc:{}", unit.as_str())] {
            let document = bar(
                r#"{"t":1705276800000},{"t":1706918400000}"#,
                &format!(r#"{{"x":{{"field":"t","type":"temporal","timeUnit":"{keyword}"}},"y":{{"field":"t","type":"temporal"}}}}"#),
            );
            let chart = vega_lite::lower(&document).unwrap_or_else(|error| panic!("`{keyword}` must be accepted: {}", error.message()));
            assert_eq!(chart.spec.transforms, Transforms { x: Some(ChannelTransform::TimeUnit(unit)), y: None });
        }
    }
}

#[test]
fn unlisted_time_units_and_local_prefixes_are_refused_with_a_path() {
    for keyword in ["quarter", "dayofyear", "week", "local:yearmonth"] {
        let document = bar(
            r#"{"t":1705276800000}"#,
            &format!(r#"{{"x":{{"field":"t","type":"temporal","timeUnit":"{keyword}"}},"y":{{"field":"t","type":"temporal"}}}}"#),
        );
        let error = vega_lite::lower(&document).expect_err("this unit is outside the subset");
        assert_eq!(error.code(), codes::UNSUPPORTED_ENCODING);
        assert_eq!(error.path(), Some("$.encoding.x.timeUnit"));
        assert!(error.span().is_some());
    }
}

#[test]
fn unsupported_bin_options_are_refused_before_any_scene_is_built() {
    let cases = [
        (
            r#"{"x":{"field":"v","type":"quantitative","bin":{"step":5,"maxbins":10}},"y":{"field":"v","type":"quantitative"}}"#,
            "$.encoding.x.bin",
        ),
        (
            r#"{"x":{"field":"v","type":"quantitative","bin":{"maxbins":1}},"y":{"field":"v","type":"quantitative"}}"#,
            "$.encoding.x.bin.maxbins",
        ),
        (
            r#"{"x":{"field":"v","type":"quantitative","bin":{"extent":[10,0]}},"y":{"field":"v","type":"quantitative"}}"#,
            "$.encoding.x.bin.extent",
        ),
        (
            r#"{"x":{"field":"v","type":"quantitative","bin":{"bins":5}},"y":{"field":"v","type":"quantitative"}}"#,
            "$.encoding.x.bin.bins",
        ),
        (
            r#"{"x":{"field":"v","type":"quantitative","bin":true,"timeUnit":"year"},"y":{"field":"v","type":"quantitative"}}"#,
            "$.encoding.x",
        ),
    ];
    for (encoding, path) in cases {
        let document = bar(r#"{"v":0},{"v":24}"#, encoding);
        let error = vega_lite::lower(&document).expect_err("this combination is refused");
        assert_eq!(error.path(), Some(path), "{}", error.message());
        assert!(error.span().is_some(), "{}", error.message());
    }
}

#[test]
fn a_bin_channel_that_is_also_an_aggregate_is_refused_by_the_spec() {
    let document = bar(
        r#"{"v":0},{"v":24}"#,
        r#"{"x":{"field":"v","type":"quantitative","bin":true,"aggregate":"sum"},"y":{"field":"v","type":"quantitative"}}"#,
    );
    let error = vega_lite::lower(&document).expect_err("a transformed channel is a grouping key");
    assert_eq!(error.code(), codes::INVALID_SPEC);
    assert!(error.message().contains("grouping key"), "{}", error.message());
}

#[test]
fn time_unit_labels_do_not_depend_on_the_process_time_zone() {
    let exe = std::env::current_exe().expect("the test binary path");
    let run = |timezone: &str| {
        let output = std::process::Command::new(&exe)
            .args(["--exact", "transforms::tz_probe_labels", "--nocapture", "--test-threads=1"])
            .env("TZ", timezone)
            .output()
            .expect("the probe test binary runs");
        assert!(output.status.success(), "the probe must pass under TZ={timezone}");
        let stdout = String::from_utf8(output.stdout).expect("probe output is UTF-8");
        stdout
            .lines()
            .filter(|line| line.starts_with("PROBE "))
            .map(str::to_string)
            .collect::<Vec<_>>()
    };
    let utc = run("UTC");
    let shanghai = run("Asia/Shanghai");
    assert!(!utc.is_empty(), "the probe must report labels");
    assert_eq!(utc, shanghai, "calendar truncation must not consult the host time zone");
}

/// Child case for [`time_unit_labels_do_not_depend_on_the_process_time_zone`].
///
/// It prints the labels of a fixed instant under every supported unit. The
/// parent runs it twice with different `TZ` values and compares the output.
#[test]
fn tz_probe_labels() {
    for unit in TimeUnit::ALL {
        println!("PROBE {} {}", unit.as_str(), unit.label(1_705_276_800_000).expect("the instant is in range"));
    }
}
