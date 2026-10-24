use davinci_chart::{
    Aggregate, Channel, ChannelEncoding, ChartOptions, ChartSpec, FieldKind, FieldRef, Mark, MarkGeometry, Scale, codes, compile,
};

use crate::support::{approx, bar_data, bar_spec, column, has_code, numbers, snapshot, texts};

fn options() -> ChartOptions {
    ChartOptions::default()
}

fn vertical_baseline(scale: &davinci_chart::LinearScale) -> f64 {
    if scale.domain.0 <= 0.0 && 0.0 <= scale.domain.1 { scale.map(0.0) } else { scale.map(scale.domain.0) }
}

#[test]
fn bar_geometry_reconciles_with_scales_and_data() {
    let plan = compile(&bar_spec(), &bar_data(), &options()).expect("bar chart should compile");
    let band = plan.scale(Channel::X).and_then(Scale::band).expect("x band scale");
    let linear = plan.scale(Channel::Y).and_then(Scale::linear).expect("y linear scale");
    let baseline = vertical_baseline(linear);

    assert_eq!(plan.marks.len(), 3, "one bar per row");
    for (instance, (category, value)) in plan.marks.iter().zip([("A", 10.0), ("B", 20.0), ("C", 5.0)]) {
        let MarkGeometry::Rect(rect) = &instance.geometry
        else {
            panic!("a bar must compile to a rectangle");
        };
        assert_eq!(instance.mark, Mark::Bar);
        assert_eq!(instance.source, category);
        assert!(approx(rect.x, band.position(category).expect("category")));
        assert!(approx(rect.width, band.band_width));

        let position = linear.map(value);
        assert!(approx(rect.y, position.min(baseline)), "bar {category} must start at the value or the baseline");
        assert!(approx(rect.height, (position - baseline).abs()), "bar {category} height must reconcile with the scale");
        assert!(rect.x >= plan.plot.x && rect.right() <= plan.plot.right(), "bars must stay inside the plot");
    }
}

#[test]
fn axis_ticks_sit_at_band_centers() {
    let plan = compile(&bar_spec(), &bar_data(), &options()).expect("bar chart should compile");
    let band = plan.scale(Channel::X).and_then(Scale::band).expect("x band scale");

    let x_axis = plan.axes.iter().find(|axis| axis.channel == Channel::X).expect("x axis");
    assert_eq!(x_axis.ticks.len(), 3);
    for (index, tick) in x_axis.ticks.iter().enumerate() {
        let category = ["A", "B", "C"][index];
        assert_eq!(tick.label, category);
        assert!(approx(tick.position.x, band.center(category).expect("category")));
        assert!(approx(tick.position.y, plan.plot.bottom()));
        assert!(tick.label_position.y > plan.plot.bottom(), "x labels sit below the axis");
    }

    let y_axis = plan.axes.iter().find(|axis| axis.channel == Channel::Y).expect("y axis");
    assert!(!y_axis.ticks.is_empty());
    for tick in &y_axis.ticks {
        assert!(tick.position.y >= plan.plot.y - 1e-9 && tick.position.y <= plan.plot.bottom() + 1e-9);
        assert!(tick.label_position.x < plan.plot.x, "y labels sit left of the axis");
    }
}

#[test]
fn count_collapses_rows_and_keeps_bar_geometry() {
    let data = snapshot(vec![column("category", FieldKind::Nominal, texts(&["A", "A", "B"]))]);
    let spec = ChartSpec::new(Mark::Bar)
        .with_field(FieldRef::new("category", FieldKind::Nominal))
        .with_encoding(Channel::X, ChannelEncoding::field("category", FieldKind::Nominal))
        .with_encoding(Channel::Y, ChannelEncoding::field("category", FieldKind::Nominal).with_aggregate(Aggregate::Count));

    let plan = compile(&spec, &data, &options()).expect("count chart should compile");
    let linear = plan.scale(Channel::Y).and_then(Scale::linear).expect("y linear scale");

    assert_eq!(plan.marks.len(), 2, "three rows collapse into two groups");
    assert_eq!(linear.domain, (1.0, 2.0));

    let heights: Vec<f64> = plan
        .marks
        .iter()
        .map(|instance| match &instance.geometry {
            MarkGeometry::Rect(rect) => rect.height,
            _ => panic!("a bar must compile to a rectangle"),
        })
        .collect();
    assert!(approx(heights[0], plan.plot.height), "count 2 maps to the top of the domain");
    assert!(approx(heights[1], 0.0), "count 1 maps to the baseline");
}

#[test]
fn sum_mean_min_and_max_change_bar_geometry() {
    let data = snapshot(vec![
        column("category", FieldKind::Nominal, texts(&["A", "A", "A"])),
        column("value", FieldKind::Quantitative, numbers(&[5.0, 10.0, 15.0])),
    ]);

    for (aggregate, expected) in [(Aggregate::Sum, 30.0), (Aggregate::Mean, 10.0), (Aggregate::Min, 5.0), (Aggregate::Max, 15.0)] {
        let spec = bar_spec().with_encoding(Channel::Y, ChannelEncoding::field("value", FieldKind::Quantitative).with_aggregate(aggregate));
        let plan = compile(&spec, &data, &options()).expect("aggregated chart should compile");
        let linear = plan.scale(Channel::Y).and_then(Scale::linear).expect("y linear scale");
        let baseline = vertical_baseline(linear);

        assert_eq!(plan.marks.len(), 1, "one group survives aggregation");
        let MarkGeometry::Rect(rect) = &plan.marks[0].geometry
        else {
            panic!("a bar must compile to a rectangle");
        };
        let position = linear.map(expected);
        assert!(approx(rect.y, position.min(baseline)), "aggregate {aggregate:?} must place the bar top");
        assert!(approx(rect.height, (position - baseline).abs()), "aggregate {aggregate:?} must place the bar height");
    }
}

#[test]
fn line_series_order_points_by_x_and_split_by_series() {
    let data = snapshot(vec![
        column("x", FieldKind::Quantitative, numbers(&[2.0, 1.0, 2.0])),
        column("y", FieldKind::Quantitative, numbers(&[20.0, 10.0, 5.0])),
        column("series", FieldKind::Nominal, texts(&["a", "a", "b"])),
    ]);
    let spec = ChartSpec::new(Mark::Line)
        .with_field(FieldRef::new("x", FieldKind::Quantitative))
        .with_field(FieldRef::new("y", FieldKind::Quantitative))
        .with_field(FieldRef::new("series", FieldKind::Nominal))
        .with_encoding(Channel::X, ChannelEncoding::field("x", FieldKind::Quantitative))
        .with_encoding(Channel::Y, ChannelEncoding::field("y", FieldKind::Quantitative))
        .with_encoding(Channel::Color, ChannelEncoding::field("series", FieldKind::Nominal));

    let plan = compile(&spec, &data, &options()).expect("line chart should compile");
    let x_scale = plan.scale(Channel::X).and_then(Scale::linear).expect("x linear scale");
    let y_scale = plan.scale(Channel::Y).and_then(Scale::linear).expect("y linear scale");

    assert_eq!(plan.marks.len(), 2, "one polyline per series");
    assert_eq!(plan.marks[0].source, "a");
    let MarkGeometry::Polyline(points) = &plan.marks[0].geometry
    else {
        panic!("a line must compile to a polyline");
    };
    assert_eq!(points.len(), 2);
    assert!(approx(points[0].x, x_scale.map(1.0)));
    assert!(approx(points[0].y, y_scale.map(10.0)));
    assert!(points[0].x < points[1].x, "points must be ordered by x");
    assert!(approx(points[1].y, y_scale.map(20.0)));
}

#[test]
fn area_series_close_the_polygon_on_the_baseline() {
    let data =
        snapshot(vec![column("x", FieldKind::Quantitative, numbers(&[1.0, 2.0])), column("y", FieldKind::Quantitative, numbers(&[3.0, 7.0]))]);
    let spec = ChartSpec::new(Mark::Area)
        .with_field(FieldRef::new("x", FieldKind::Quantitative))
        .with_field(FieldRef::new("y", FieldKind::Quantitative))
        .with_encoding(Channel::X, ChannelEncoding::field("x", FieldKind::Quantitative))
        .with_encoding(Channel::Y, ChannelEncoding::field("y", FieldKind::Quantitative));

    let plan = compile(&spec, &data, &options()).expect("area chart should compile");
    let y_scale = plan.scale(Channel::Y).and_then(Scale::linear).expect("y linear scale");
    let baseline = vertical_baseline(y_scale);

    let MarkGeometry::Polygon(points) = &plan.marks[0].geometry
    else {
        panic!("an area must compile to a polygon");
    };
    assert_eq!(points.len(), 4, "two data points plus two baseline corners");
    assert!(approx(points[2].y, baseline));
    assert!(approx(points[3].y, baseline));
    assert!(approx(points[2].x, points[1].x));
    assert!(approx(points[3].x, points[0].x));
}

#[test]
fn rules_span_the_plot_and_text_marks_anchor_at_their_point() {
    let data = snapshot(vec![column("value", FieldKind::Quantitative, numbers(&[7.0]))]);
    let rule = ChartSpec::new(Mark::Rule)
        .with_field(FieldRef::new("value", FieldKind::Quantitative))
        .with_encoding(Channel::Y, ChannelEncoding::field("value", FieldKind::Quantitative));
    let plan = compile(&rule, &data, &options()).expect("rule chart should compile");
    let linear = plan.scale(Channel::Y).and_then(Scale::linear).expect("y linear scale");
    let MarkGeometry::Segment { from, to } = &plan.marks[0].geometry
    else {
        panic!("a rule must compile to a segment");
    };
    assert!(approx(from.x, plan.plot.x));
    assert!(approx(to.x, plan.plot.right()));
    assert!(approx(from.y, linear.map(7.0)));

    let text = ChartSpec::new(Mark::Text)
        .with_field(FieldRef::new("x", FieldKind::Quantitative))
        .with_field(FieldRef::new("value", FieldKind::Quantitative))
        .with_encoding(Channel::X, ChannelEncoding::field("x", FieldKind::Quantitative))
        .with_encoding(Channel::Y, ChannelEncoding::field("value", FieldKind::Quantitative));
    let data = snapshot(vec![column("x", FieldKind::Quantitative, numbers(&[1.0])), column("value", FieldKind::Quantitative, numbers(&[7.0]))]);
    let plan = compile(&text, &data, &options()).expect("text chart should compile");
    let MarkGeometry::Label { anchor, text } = &plan.marks[0].geometry
    else {
        panic!("a text mark must compile to a label");
    };
    assert_eq!(text, "7");
    assert!(anchor.x > plan.plot.x && anchor.y > plan.plot.y);
}

#[test]
fn explicit_bands_and_linear_domains_are_honoured() {
    let spec = bar_spec()
        .with_scale(Channel::X, davinci_chart::ScaleSpec::band(0.5))
        .with_scale(Channel::Y, davinci_chart::ScaleSpec::linear_domain(0.0, 100.0));
    let plan = compile(&spec, &bar_data(), &options()).expect("explicit scales should compile");

    let band = plan.scale(Channel::X).and_then(Scale::band).expect("x band scale");
    assert!(approx(band.padding, 0.5));
    let linear = plan.scale(Channel::Y).and_then(Scale::linear).expect("y linear scale");
    assert_eq!(linear.domain, (0.0, 100.0));
    assert!(approx(linear.map(20.0), vertical_baseline(linear) - plan.plot.height * 0.2));
}

#[test]
fn unknown_snapshot_fields_fail_before_scaling() {
    let data = snapshot(vec![column("category", FieldKind::Nominal, texts(&["A"])), column("other", FieldKind::Quantitative, numbers(&[1.0]))]);

    let error = compile(&bar_spec(), &data, &options()).expect_err("the field is missing");
    assert_eq!(error.code(), codes::UNKNOWN_FIELD);
}

#[test]
fn a_snapshot_with_no_usable_rows_is_rejected() {
    let data = snapshot(vec![
        column("category", FieldKind::Nominal, vec![davinci_chart::Value::Null]),
        column("value", FieldKind::Quantitative, vec![davinci_chart::Value::Null]),
    ]);

    let error = compile(&bar_spec(), &data, &options()).expect_err("no usable rows");
    assert_eq!(error.code(), codes::EMPTY_DOMAIN);
}

#[test]
fn ragged_data_is_rejected_with_the_validation_code() {
    let data =
        snapshot(vec![column("category", FieldKind::Nominal, texts(&["A", "B"])), column("value", FieldKind::Quantitative, numbers(&[1.0]))]);

    let error = compile(&bar_spec(), &data, &options()).expect_err("ragged data");
    assert_eq!(error.code(), codes::RAGGED_COLUMN);
    assert_eq!(error.diagnostic().code().as_str(), codes::RAGGED_COLUMN);
}

#[test]
fn empty_guides_suppress_axes_and_legends_are_reported() {
    let spec = bar_spec().with_guides(vec![davinci_chart::Guide::legend(Channel::Color)]);
    let plan = compile(&spec, &bar_data(), &options()).expect("bar chart should compile");

    assert!(plan.axes.is_empty(), "no axis guides means no axes");
    assert!(has_code(&plan.diagnostics, codes::APPROXIMATED), "legend guides are reported");
}

#[test]
fn compiling_the_same_input_twice_is_identical() {
    let first = compile(&bar_spec(), &bar_data(), &options()).expect("bar chart");
    let second = compile(&bar_spec(), &bar_data(), &options()).expect("bar chart");
    assert_eq!(first, second);
}

#[test]
fn point_positions_reconcile_with_both_scales() {
    let data = snapshot(vec![column("x", FieldKind::Quantitative, numbers(&[1.0])), column("y", FieldKind::Quantitative, numbers(&[2.0]))]);
    let spec = ChartSpec::new(Mark::Point)
        .with_field(FieldRef::new("x", FieldKind::Quantitative))
        .with_field(FieldRef::new("y", FieldKind::Quantitative))
        .with_encoding(Channel::X, ChannelEncoding::field("x", FieldKind::Quantitative))
        .with_encoding(Channel::Y, ChannelEncoding::field("y", FieldKind::Quantitative));
    let plan = compile(&spec, &data, &options()).expect("point chart should compile");
    let x_scale = plan.scale(Channel::X).and_then(Scale::linear).expect("x linear scale");
    let y_scale = plan.scale(Channel::Y).and_then(Scale::linear).expect("y linear scale");

    let MarkGeometry::Point { center, .. } = &plan.marks[0].geometry
    else {
        panic!("a point must compile to a marker");
    };
    assert!(approx(center.x, x_scale.map(1.0)));
    assert!(approx(center.y, y_scale.map(2.0)));
    assert!(center.x.is_finite() && center.y.is_finite(), "point geometry must never be NaN");
}
