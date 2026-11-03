//! `bin` and `timeUnit` are chart transforms, not mark options.
//!
//! A transformed positional channel becomes an ordered categorical domain, so
//! the assertions here read the trained band/point scale and the axis labels
//! produced from the derived buckets rather than any document syntax.

use davinci_chart::{Channel, ChannelTransform, TimeUnit, Transforms, codes, vega_lite};

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
    assert!(error.span().is_some(), "the refusal keeps a range");
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
