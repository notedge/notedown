//! Deterministic compilation from chart semantics to a compile plan.
//!
//! Compilation validates both inputs, applies the declared statistic, trains
//! every scale, and produces final mark geometry plus axis ticks. It never
//! uses randomness or hash-map iteration order, and it keeps rows and
//! categories in first-appearance order.

use std::cmp::Ordering;

use davinci_scene::{Color, MarkerShape, Point, Rect, Size};
use davinci_types::{Diagnostic, DiagnosticSeverity};

use crate::{
    data::{DataSnapshot, FieldKind, Value, format_number},
    error::{ChartError, chart_warning, codes},
    plan::{AxisPlan, AxisTick, ChartCompilePlan, ChartScale, MarkGeometry, MarkInstance, MarkStyle},
    scale::{Scale, Transfer, TransformedScale, linear_ticks, log_ticks, train_band, train_linear, train_nominal, train_point},
    spec::{Channel, ChartSpec, Guide, Mark, ScaleSpec},
    statistic::{Observation, aggregate},
};

const DEFAULT_MARK_COLOR: Color = Color::rgb(0x4a, 0x7f, 0xbf);
const DEFAULT_TEXT_COLOR: Color = Color::rgb(0x2d, 0x37, 0x48);
const RULE_WIDTH: f64 = 1.0;
const TICK_MARK_LENGTH: f64 = 6.0;
const OPACITY_MIN: f64 = 0.05;
const AXIS_LABEL_GAP: f64 = 4.0;
const AXIS_LABEL_INSET: f64 = 6.0;

/// Constant paint defaults resolved once from the spec.
#[derive(Clone, Copy, Debug, PartialEq)]
struct PaintDefaults {
    color: Color,
    text_color: Color,
    opacity: f64,
}

impl PaintDefaults {
    fn new(spec: &ChartSpec) -> Self {
        Self {
            color: spec.mark_color.unwrap_or(DEFAULT_MARK_COLOR),
            text_color: spec.mark_color.unwrap_or(DEFAULT_TEXT_COLOR),
            opacity: spec.mark_opacity.unwrap_or(1.0),
        }
    }
}

/// Tuning for deterministic chart compilation.
#[derive(Clone, Debug, PartialEq)]
pub struct ChartOptions {
    /// Canvas width.
    pub width: f64,
    /// Canvas height.
    pub height: f64,
    /// Empty space between the canvas edge and the plot rectangle.
    pub margin: f64,
    /// Font size used for axis labels.
    pub font_size: f64,
    /// Default point diameter when no size channel is bound.
    pub point_size: f64,
    /// Stroke width used for line series.
    pub stroke_width: f64,
    /// Requested number of linear axis ticks.
    pub tick_count: usize,
}

impl Default for ChartOptions {
    fn default() -> Self {
        Self { width: 640.0, height: 400.0, margin: 56.0, font_size: 12.0, point_size: 10.0, stroke_width: 2.0, tick_count: 5 }
    }
}

/// Compile a chart spec and a data snapshot into a deterministic plan.
///
/// Documented degradation rules:
///
/// - Missing values are reported and affected rows are skipped.
/// - A linear domain with one distinct value is expanded and reported.
/// - A quantitative color or shape channel is treated as a category.
/// - Legend guides and non-positional axis guides are reported, not drawn.
///
/// Structural failures, such as ragged columns, kind mismatches, unknown
/// fields, and channels with no values, return a [`ChartError`].
pub fn compile(spec: &ChartSpec, data: &DataSnapshot, options: &ChartOptions) -> Result<ChartCompilePlan, ChartError> {
    if !(options.width.is_finite() && options.height.is_finite() && options.width > 0.0 && options.height > 0.0) {
        return Err(ChartError::new(codes::INVALID_SPEC, "chart canvas width and height must be positive finite numbers"));
    }

    let mut diagnostics = Vec::new();
    collect_or_fail(data.validate(), &mut diagnostics)?;
    collect_or_fail(spec.validate(), &mut diagnostics)?;
    check_fields(spec, data)?;

    let measure = measure_channel(spec)?;
    let mut rows = project_rows(spec, data, measure, &mut diagnostics)?;
    if let Some(channel) = measure {
        rows = aggregate_rows(spec, channel, rows, &mut diagnostics);
    }
    if rows.is_empty() {
        return Err(ChartError::new(codes::EMPTY_DOMAIN, "the chart has no usable rows after projection and aggregation"));
    }

    let plot = plot_rect(options);
    let mut scales = Vec::new();
    for channel in Channel::ALL {
        let Some(encoding) = spec.encoding.channel(channel)
        else {
            continue;
        };
        let declared = spec.scales.channel(channel).copied().unwrap_or_default();
        let scale = train_channel(
            channel,
            encoding.field.kind,
            encoding.aggregate.is_some(),
            declared,
            spec.mark,
            &rows,
            plot,
            options,
            &mut diagnostics,
        )?;
        scales.push(ChartScale { channel, scale });
    }

    let paints = PaintDefaults::new(spec);
    let mut marks = Vec::new();
    match spec.mark {
        Mark::Bar => build_bars(&rows, &scales, paints, &mut marks)?,
        Mark::Point => build_points(&rows, &scales, options, paints, &mut marks),
        Mark::Line | Mark::Area => build_series(spec.mark, &rows, &scales, options, paints, &mut marks)?,
        Mark::Rule => build_rules(&rows, &scales, &plot, paints, &mut marks)?,
        Mark::Rect => build_rects(&rows, &scales, paints, &mut marks)?,
        Mark::Tick => build_ticks(&rows, &scales, paints, &mut marks)?,
        Mark::Text => build_texts(&rows, &scales, paints, &mut marks),
    }

    let mut axes: Vec<AxisPlan> = Vec::new();
    for guide in &spec.guides {
        match *guide {
            Guide::Axis { channel } => {
                if !matches!(channel, Channel::X | Channel::Y) {
                    diagnostics.push(chart_warning(
                        codes::APPROXIMATED,
                        format!("axis guide on the `{}` channel has no lowering in this slice", channel.as_str()),
                    ));
                    continue;
                }
                if axes.iter().any(|axis| axis.channel == channel) {
                    continue;
                }
                match build_axis(channel, &scales, &plot, options) {
                    Some(axis) => axes.push(axis),
                    None => diagnostics.push(chart_warning(
                        codes::APPROXIMATED,
                        format!("axis guide on the `{}` channel has no trainable scale", channel.as_str()),
                    )),
                }
            }
            Guide::Legend { channel } => diagnostics.push(chart_warning(
                codes::APPROXIMATED,
                format!("legend guide on the `{}` channel is not rendered in this slice", channel.as_str()),
            )),
        }
    }

    Ok(ChartCompilePlan {
        canvas: Size::new(options.width, options.height),
        plot,
        font_size: options.font_size,
        scales,
        marks,
        axes,
        diagnostics,
    })
}

fn collect_or_fail(records: Vec<Diagnostic>, diagnostics: &mut Vec<Diagnostic>) -> Result<(), ChartError> {
    for record in records {
        if record.severity() == DiagnosticSeverity::Error {
            return Err(ChartError::from_diagnostic(&record));
        }
        diagnostics.push(record);
    }
    Ok(())
}

fn check_fields(spec: &ChartSpec, data: &DataSnapshot) -> Result<(), ChartError> {
    for field in &spec.fields {
        match data.field(&field.name) {
            None => {
                return Err(ChartError::new(codes::UNKNOWN_FIELD, format!("field `{}` is not in the data snapshot", field.name)));
            }
            Some(column) if column.kind != field.kind => {
                return Err(ChartError::new(
                    codes::TYPE_MISMATCH,
                    format!(
                        "field `{}` is declared as {} but the snapshot column is {}",
                        field.name,
                        field.kind.as_str(),
                        column.kind.as_str()
                    ),
                ));
            }
            Some(_) => {}
        }
    }
    Ok(())
}

fn measure_channel(spec: &ChartSpec) -> Result<Option<Channel>, ChartError> {
    let mut measure = None;
    for channel in Channel::ALL {
        if let Some(encoding) = spec.encoding.channel(channel) {
            if encoding.aggregate.is_none() {
                continue;
            }
            if measure.is_some() {
                return Err(ChartError::new(codes::INVALID_SPEC, "only one aggregated channel is supported per chart"));
            }
            measure = Some(channel);
        }
    }
    Ok(measure)
}

fn project_rows(
    spec: &ChartSpec,
    data: &DataSnapshot,
    measure: Option<Channel>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<Vec<Row>, ChartError> {
    let mut missing = [0usize; 6];
    let mut rows = Vec::new();
    for index in 0..data.row_count() {
        let mut row = Row::default();
        let mut complete = true;
        for channel in Channel::ALL {
            let Some(encoding) = spec.encoding.channel(channel)
            else {
                continue;
            };
            let column = data
                .field(&encoding.field.name)
                .ok_or_else(|| ChartError::new(codes::UNKNOWN_FIELD, format!("field `{}` is not in the data snapshot", encoding.field.name)))?;
            let value = column.values[index].clone();
            if value.is_null() {
                missing[channel_index(channel)] += 1;
                if Some(channel) == measure {
                    row.set(channel, value);
                    continue;
                }
                complete = false;
                continue;
            }
            row.set(channel, value);
        }
        if complete {
            rows.push(row);
        }
    }

    for channel in Channel::ALL {
        let count = missing[channel_index(channel)];
        if count == 0 {
            continue;
        }
        let name = spec.encoding.channel(channel).map(|encoding| encoding.field.name.as_str()).unwrap_or_default();
        diagnostics
            .push(chart_warning(codes::MISSING_VALUE, format!("field `{name}` has {count} missing value(s); affected rows are skipped")));
    }
    Ok(rows)
}

fn aggregate_rows(spec: &ChartSpec, channel: Channel, rows: Vec<Row>, diagnostics: &mut Vec<Diagnostic>) -> Vec<Row> {
    let encoding = spec.encoding.channel(channel).expect("a measured channel is always bound");
    let aggregate_kind = encoding.aggregate.expect("a measured channel always declares an aggregate");
    let observations: Vec<Observation> = rows.iter().map(|row| Observation::new(row.keys(Some(channel)), row.get(channel).cloned())).collect();
    let aggregated = aggregate(&observations, aggregate_kind);

    let mut output = Vec::with_capacity(aggregated.len());
    let mut dropped = 0usize;
    for entry in aggregated {
        if entry.measure.is_null() {
            dropped += 1;
            continue;
        }
        let mut row = Row::default();
        let mut keys = entry.keys.into_iter();
        for key_channel in Channel::ALL.into_iter().filter(|key_channel| *key_channel != channel) {
            if spec.encoding.channel(key_channel).is_none() {
                continue;
            }
            if let Some(value) = keys.next() {
                row.set(key_channel, value);
            }
        }
        row.set(channel, entry.measure);
        output.push(row);
    }
    if dropped > 0 {
        diagnostics.push(chart_warning(codes::MISSING_VALUE, format!("{dropped} aggregated group(s) have no numeric value and were skipped")));
    }
    output
}

fn train_channel(
    channel: Channel,
    kind: FieldKind,
    aggregated: bool,
    declared: ScaleSpec,
    mark: Mark,
    rows: &[Row],
    plot: Rect,
    options: &ChartOptions,
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<Scale, ChartError> {
    let effective = if aggregated { FieldKind::Quantitative } else { kind };
    match channel {
        Channel::Color | Channel::Shape => {
            let categories = categories(rows, channel);
            if categories.is_empty() {
                return Err(empty_domain(channel));
            }
            Ok(Scale::Nominal(train_nominal(&categories)))
        }
        Channel::Size => {
            let values = numbers(rows, channel);
            if values.is_empty() {
                return Err(empty_domain(channel));
            }
            if collapsed(&values) {
                diagnostics.push(chart_warning(
                    codes::DEGENERATE_DOMAIN,
                    format!("the `{}` channel has one distinct value; its linear domain was expanded", channel.as_str()),
                ));
            }
            let range = (options.point_size * 0.5, options.point_size * 2.0);
            Ok(Scale::Linear(train_linear(&values, None, range)))
        }
        Channel::Opacity => {
            let values = numbers(rows, channel);
            if values.is_empty() {
                return Err(empty_domain(channel));
            }
            if collapsed(&values) {
                diagnostics.push(chart_warning(
                    codes::DEGENERATE_DOMAIN,
                    format!("the `{}` channel has one distinct value; its linear domain was expanded", channel.as_str()),
                ));
            }
            Ok(Scale::Linear(train_linear(&values, None, (OPACITY_MIN, 1.0))))
        }
        Channel::X | Channel::Y => {
            let range = if channel == Channel::X { (plot.x, plot.right()) } else { (plot.bottom(), plot.y) };
            let requested = match declared {
                ScaleSpec::Auto => default_spec(effective, mark),
                other => other,
            };
            match requested {
                ScaleSpec::Linear { domain } => {
                    let values = numbers(rows, channel);
                    if values.is_empty() {
                        return Err(empty_domain(channel));
                    }
                    if domain.is_none() && collapsed(&values) {
                        diagnostics.push(chart_warning(
                            codes::DEGENERATE_DOMAIN,
                            format!("the `{}` channel has one distinct value; its linear domain was expanded", channel.as_str()),
                        ));
                    }
                    Ok(Scale::Linear(train_linear(&values, domain, range)))
                }
                ScaleSpec::Log { domain } => train_transformed(rows, channel, domain, range, Transfer::Log, diagnostics),
                ScaleSpec::Sqrt { domain } => train_transformed(rows, channel, domain, range, Transfer::Sqrt, diagnostics),
                ScaleSpec::Band { padding } => {
                    let categories = categories(rows, channel);
                    if categories.is_empty() {
                        return Err(empty_domain(channel));
                    }
                    Ok(Scale::Band(train_band(&categories, range, padding)))
                }
                ScaleSpec::Point { padding } => {
                    let categories = categories(rows, channel);
                    if categories.is_empty() {
                        return Err(empty_domain(channel));
                    }
                    Ok(Scale::Point(train_point(&categories, range, padding)))
                }
                ScaleSpec::Auto | ScaleSpec::Nominal => {
                    Err(ChartError::new(codes::INVALID_SCALE, format!("the `{}` channel cannot use a nominal scale", channel.as_str())))
                }
            }
        }
    }
}

fn train_transformed(
    rows: &[Row],
    channel: Channel,
    domain: Option<(f64, f64)>,
    range: (f64, f64),
    transfer: Transfer,
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<Scale, ChartError> {
    let values = numbers(rows, channel);
    if values.is_empty() {
        return Err(empty_domain(channel));
    }
    let strictly_positive = transfer == Transfer::Log;
    for value in &values {
        let invalid = if strictly_positive { *value <= 0.0 } else { *value < 0.0 };
        if invalid {
            return Err(ChartError::new(
                codes::INVALID_SCALE,
                format!(
                    "the `{}` channel has value `{value}`, a {} scale needs {} values",
                    channel.as_str(),
                    transfer.as_str(),
                    if strictly_positive { "strictly positive" } else { "non-negative" }
                ),
            ));
        }
    }
    let (min, max) = match domain {
        Some((min, max)) => (min, max),
        None => {
            let min = values.iter().copied().fold(f64::INFINITY, f64::min);
            let max = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            if min == max {
                diagnostics.push(chart_warning(
                    codes::DEGENERATE_DOMAIN,
                    format!(
                        "the `{}` channel has one distinct value; its {} domain was expanded",
                        channel.as_str(),
                        transfer.as_str()
                    ),
                ));
                match transfer {
                    Transfer::Log => (min * 0.5, max * 2.0),
                    Transfer::Sqrt if min == 0.0 => (0.0, 1.0),
                    Transfer::Sqrt => (min * 0.5, max * 1.5),
                }
            }
            else {
                (min, max)
            }
        }
    };
    Ok(Scale::Transformed(TransformedScale::new(transfer, (min, max), range)))
}

fn default_spec(kind: FieldKind, mark: Mark) -> ScaleSpec {
    if kind.is_quantitative() {
        ScaleSpec::linear()
    }
    else if matches!(mark, Mark::Bar | Mark::Rect) {
        ScaleSpec::band(0.1)
    }
    else {
        ScaleSpec::point(0.5)
    }
}

fn numbers(rows: &[Row], channel: Channel) -> Vec<f64> {
    rows.iter().filter_map(|row| row.get(channel).and_then(Value::as_number)).filter(|value| value.is_finite()).collect()
}

fn categories(rows: &[Row], channel: Channel) -> Vec<String> {
    let mut categories: Vec<String> = Vec::new();
    for row in rows {
        let Some(value) = row.get(channel)
        else {
            continue;
        };
        let label = value.label();
        if !categories.contains(&label) {
            categories.push(label);
        }
    }
    categories
}

fn collapsed(values: &[f64]) -> bool {
    values.iter().all(|value| *value == values[0])
}

fn empty_domain(channel: Channel) -> ChartError {
    ChartError::new(codes::EMPTY_DOMAIN, format!("the `{}` channel has no values to train its scale", channel.as_str()))
}

fn plot_rect(options: &ChartOptions) -> Rect {
    let margin = options.margin.max(0.0);
    Rect::new(margin, margin, (options.width - margin * 2.0).max(1.0), (options.height - margin * 2.0).max(1.0))
}

fn find_scale(scales: &[ChartScale], channel: Channel) -> Option<&Scale> {
    scales.iter().find(|entry| entry.channel == channel).map(|entry| &entry.scale)
}

fn map_value(scale: &Scale, value: &Value) -> Option<f64> {
    match scale {
        Scale::Linear(linear) => value.as_number().map(|number| linear.map(number)),
        Scale::Transformed(transformed) => value.as_number().map(|number| transformed.map(number)),
        Scale::Band(band) => band.center(&value.label()),
        Scale::Point(point) => point.position(&value.label()),
        Scale::Nominal(_) => None,
    }
}

fn position_of(row: &Row, scales: &[ChartScale]) -> Option<Point> {
    let x = find_scale(scales, Channel::X).and_then(|scale| map_value(scale, row.get(Channel::X)?))?;
    let y = find_scale(scales, Channel::Y).and_then(|scale| map_value(scale, row.get(Channel::Y)?))?;
    Some(Point::new(x, y))
}

fn numeric(row: &Row, channel: Channel) -> Result<f64, ChartError> {
    row.get(channel)
        .and_then(Value::as_number)
        .filter(|value| value.is_finite())
        .ok_or_else(|| ChartError::new(codes::TYPE_MISMATCH, format!("the `{}` channel needs a finite numeric value", channel.as_str())))
}

fn color_for(row: &Row, scales: &[ChartScale], paints: PaintDefaults) -> Color {
    find_scale(scales, Channel::Color)
        .and_then(Scale::nominal)
        .and_then(|nominal| row.get(Channel::Color).and_then(|value| nominal.color(&value.label())))
        .unwrap_or(paints.color)
}

fn shape_for(row: &Row, scales: &[ChartScale]) -> MarkerShape {
    find_scale(scales, Channel::Shape)
        .and_then(Scale::nominal)
        .and_then(|nominal| row.get(Channel::Shape).and_then(|value| nominal.marker(&value.label())))
        .unwrap_or(MarkerShape::Circle)
}

fn size_for(row: &Row, scales: &[ChartScale], options: &ChartOptions) -> f64 {
    find_scale(scales, Channel::Size)
        .and_then(Scale::linear)
        .and_then(|linear| row.get(Channel::Size).and_then(Value::as_number).map(|value| linear.map(value)))
        .unwrap_or(options.point_size)
}

fn opacity_for(row: &Row, scales: &[ChartScale], paints: PaintDefaults) -> f64 {
    match find_scale(scales, Channel::Opacity)
        .and_then(Scale::linear)
        .and_then(|linear| row.get(Channel::Opacity).and_then(Value::as_number).map(|value| linear.map(value)))
    {
        Some(value) => value.clamp(OPACITY_MIN, 1.0),
        None => paints.opacity,
    }
}

fn filled_style(row: &Row, scales: &[ChartScale], paints: PaintDefaults) -> MarkStyle {
    MarkStyle { opacity: opacity_for(row, scales, paints), ..MarkStyle::filled(color_for(row, scales, paints)) }
}

fn stroked_style(row: &Row, scales: &[ChartScale], paints: PaintDefaults, width: f64) -> MarkStyle {
    MarkStyle { opacity: opacity_for(row, scales, paints), ..MarkStyle::stroked(color_for(row, scales, paints), width) }
}

fn source_for(row: &Row) -> String {
    row.get(Channel::X).map(Value::label).or_else(|| row.get(Channel::Y).map(Value::label)).unwrap_or_else(|| "row".to_string())
}

/// The baseline position used by bars and areas.
///
/// The baseline is zero when the domain contains it and the domain minimum
/// otherwise, so bars always start inside the plot. A transformed scale
/// cannot contain zero, so it always uses the domain minimum.
fn baseline_position(scale: &Scale) -> Option<f64> {
    match scale {
        Scale::Linear(linear) => {
            let (min, max) = linear.domain;
            Some(linear.map(if min <= 0.0 && 0.0 <= max { 0.0 } else { min }))
        }
        Scale::Transformed(transformed) => Some(transformed.map(transformed.domain.0)),
        _ => None,
    }
}

fn build_bars(rows: &[Row], scales: &[ChartScale], paints: PaintDefaults, marks: &mut Vec<MarkInstance>) -> Result<(), ChartError> {
    let x = find_scale(scales, Channel::X);
    let y = find_scale(scales, Channel::Y);
    match (x, y) {
        (Some(Scale::Band(band)), Some(y_scale)) if y_scale.numeric_domain().is_some() => {
            let baseline = baseline_position(y_scale).expect("a numeric scale has a baseline");
            for (index, row) in rows.iter().enumerate() {
                let category = row.get(Channel::X).map(Value::label).unwrap_or_default();
                let start = band
                    .position(&category)
                    .ok_or_else(|| ChartError::new(codes::INVALID_SPEC, format!("category `{category}` is missing from the band scale")))?;
                let position = y_scale.map_number(numeric(row, Channel::Y)?).expect("the guard checked a numeric scale");
                let rect = Rect::new(start, position.min(baseline), band.band_width, (position - baseline).abs());
                marks.push(MarkInstance {
                    id: format!("chart-bar-{index}"),
                    mark: Mark::Bar,
                    source: category,
                    geometry: MarkGeometry::Rect(rect),
                    style: filled_style(row, scales, paints),
                });
            }
        }
        (Some(x_scale), Some(Scale::Band(band))) if x_scale.numeric_domain().is_some() => {
            let baseline = baseline_position(x_scale).expect("a numeric scale has a baseline");
            for (index, row) in rows.iter().enumerate() {
                let category = row.get(Channel::Y).map(Value::label).unwrap_or_default();
                let start = band
                    .position(&category)
                    .ok_or_else(|| ChartError::new(codes::INVALID_SPEC, format!("category `{category}` is missing from the band scale")))?;
                let position = x_scale.map_number(numeric(row, Channel::X)?).expect("the guard checked a numeric scale");
                let rect = Rect::new(position.min(baseline), start, (position - baseline).abs(), band.band_width);
                marks.push(MarkInstance {
                    id: format!("chart-bar-{index}"),
                    mark: Mark::Bar,
                    source: category,
                    geometry: MarkGeometry::Rect(rect),
                    style: filled_style(row, scales, paints),
                });
            }
        }
        _ => {
            return Err(ChartError::new(codes::INVALID_SPEC, "the bar mark needs a band scale on one axis and a continuous scale on the other"));
        }
    }
    Ok(())
}

fn build_points(rows: &[Row], scales: &[ChartScale], options: &ChartOptions, paints: PaintDefaults, marks: &mut Vec<MarkInstance>) {
    for (index, row) in rows.iter().enumerate() {
        let Some(center) = position_of(row, scales)
        else {
            continue;
        };
        marks.push(MarkInstance {
            id: format!("chart-point-{index}"),
            mark: Mark::Point,
            source: source_for(row),
            geometry: MarkGeometry::Point { center, size: size_for(row, scales, options), shape: shape_for(row, scales) },
            style: filled_style(row, scales, paints),
        });
    }
}

fn series_label(row: &Row) -> String {
    match (row.get(Channel::Color).map(Value::label), row.get(Channel::Shape).map(Value::label)) {
        (Some(color), Some(shape)) => format!("{color}|{shape}"),
        (Some(color), None) => color,
        (None, Some(shape)) => shape,
        (None, None) => "all".to_string(),
    }
}

fn order_key(row: &Row, scale: &Scale) -> f64 {
    let Some(value) = row.get(Channel::X)
    else {
        return f64::INFINITY;
    };
    match scale {
        Scale::Band(band) => band.index_of(&value.label()).map(|index| index as f64).unwrap_or(f64::INFINITY),
        Scale::Point(point) => point.index_of(&value.label()).map(|index| index as f64).unwrap_or(f64::INFINITY),
        _ => value.as_number().unwrap_or(f64::INFINITY),
    }
}

fn build_series(
    mark: Mark,
    rows: &[Row],
    scales: &[ChartScale],
    options: &ChartOptions,
    paints: PaintDefaults,
    marks: &mut Vec<MarkInstance>,
) -> Result<(), ChartError> {
    let Some(x_scale) = find_scale(scales, Channel::X)
    else {
        return Err(ChartError::new(codes::INVALID_SPEC, "the line and area marks need an `x` binding"));
    };
    let Some(y_scale) = find_scale(scales, Channel::Y).filter(|scale| scale.numeric_domain().is_some())
    else {
        return Err(ChartError::new(codes::INVALID_SPEC, "the line and area marks need a continuous `y` scale"));
    };
    let mut series: Vec<(String, Color, Vec<(f64, Point)>, f64)> = Vec::new();
    for row in rows {
        let Some(point) = position_of(row, scales)
        else {
            continue;
        };
        let label = series_label(row);
        let index = match series.iter().position(|(existing, _, _, _)| *existing == label) {
            Some(index) => index,
            None => {
                series.push((label, color_for(row, scales, paints), Vec::new(), opacity_for(row, scales, paints)));
                series.len() - 1
            }
        };
        series[index].2.push((order_key(row, x_scale), point));
    }

    for (index, (label, color, mut points, opacity)) in series.into_iter().enumerate() {
        points.sort_by(|left, right| left.0.partial_cmp(&right.0).unwrap_or(Ordering::Equal));
        let points: Vec<Point> = points.into_iter().map(|(_, point)| point).collect();
        if points.is_empty() {
            continue;
        }
        match mark {
            Mark::Area => {
                let baseline = baseline_position(y_scale).expect("the check above selected a numeric scale");
                let first = points[0];
                let last = points[points.len() - 1];
                let mut polygon = points;
                polygon.push(Point::new(last.x, baseline));
                polygon.push(Point::new(first.x, baseline));
                let style = MarkStyle { fill: Some(color), stroke: Some(color), stroke_width: RULE_WIDTH, opacity };
                marks.push(MarkInstance {
                    id: format!("chart-area-{index}"),
                    mark: Mark::Area,
                    source: label,
                    geometry: MarkGeometry::Polygon(polygon),
                    style,
                });
            }
            _ => {
                marks.push(MarkInstance {
                    id: format!("chart-line-{index}"),
                    mark: Mark::Line,
                    source: label,
                    geometry: MarkGeometry::Polyline(points),
                    style: MarkStyle { opacity, ..MarkStyle::stroked(color, options.stroke_width) },
                });
            }
        }
    }
    Ok(())
}

fn build_rules(
    rows: &[Row],
    scales: &[ChartScale],
    plot: &Rect,
    paints: PaintDefaults,
    marks: &mut Vec<MarkInstance>,
) -> Result<(), ChartError> {
    let horizontal = find_scale(scales, Channel::Y).and_then(Scale::linear);
    let vertical = find_scale(scales, Channel::X).and_then(Scale::linear);
    if horizontal.is_none() && vertical.is_none() {
        return Err(ChartError::new(codes::INVALID_SPEC, "the rule mark needs a linear `x` or `y` scale"));
    }
    for (index, row) in rows.iter().enumerate() {
        let (source, geometry) = if let Some(linear) = horizontal {
            let position = linear.map(numeric(row, Channel::Y)?);
            (
                row.get(Channel::Y).map(Value::label).unwrap_or_default(),
                MarkGeometry::Segment { from: Point::new(plot.x, position), to: Point::new(plot.right(), position) },
            )
        }
        else if let Some(linear) = vertical {
            let position = linear.map(numeric(row, Channel::X)?);
            (
                row.get(Channel::X).map(Value::label).unwrap_or_default(),
                MarkGeometry::Segment { from: Point::new(position, plot.y), to: Point::new(position, plot.bottom()) },
            )
        }
        else {
            continue;
        };
        marks.push(MarkInstance {
            id: format!("chart-rule-{index}"),
            mark: Mark::Rule,
            source,
            geometry,
            style: stroked_style(row, scales, paints, RULE_WIDTH),
        });
    }
    Ok(())
}

fn build_rects(rows: &[Row], scales: &[ChartScale], paints: PaintDefaults, marks: &mut Vec<MarkInstance>) -> Result<(), ChartError> {
    let x_band = find_scale(scales, Channel::X).and_then(Scale::band);
    let y_band = find_scale(scales, Channel::Y).and_then(Scale::band);
    let (Some(x_band), Some(y_band)) = (x_band, y_band)
    else {
        return Err(ChartError::new(codes::INVALID_SPEC, "the rect mark needs band scales on both axes"));
    };
    for (index, row) in rows.iter().enumerate() {
        let x_label = row.get(Channel::X).map(Value::label).unwrap_or_default();
        let y_label = row.get(Channel::Y).map(Value::label).unwrap_or_default();
        let x = x_band
            .position(&x_label)
            .ok_or_else(|| ChartError::new(codes::INVALID_SPEC, format!("category `{x_label}` is missing from the x band scale")))?;
        let y = y_band
            .position(&y_label)
            .ok_or_else(|| ChartError::new(codes::INVALID_SPEC, format!("category `{y_label}` is missing from the y band scale")))?;
        // A y axis runs bottom-to-top, so its band width is negative and the
        // recorded position is the band's lower edge. Normalize the rectangle.
        let (x, width) = if x_band.band_width < 0.0 { (x + x_band.band_width, -x_band.band_width) } else { (x, x_band.band_width) };
        let (y, height) = if y_band.band_width < 0.0 { (y + y_band.band_width, -y_band.band_width) } else { (y, y_band.band_width) };
        marks.push(MarkInstance {
            id: format!("chart-rect-{index}"),
            mark: Mark::Rect,
            source: format!("{x_label}|{y_label}"),
            geometry: MarkGeometry::Rect(Rect::new(x, y, width, height)),
            style: filled_style(row, scales, paints),
        });
    }
    Ok(())
}

fn build_ticks(rows: &[Row], scales: &[ChartScale], paints: PaintDefaults, marks: &mut Vec<MarkInstance>) -> Result<(), ChartError> {
    let x = find_scale(scales, Channel::X);
    let y = find_scale(scales, Channel::Y);
    let half = TICK_MARK_LENGTH * 0.5;
    for (index, row) in rows.iter().enumerate() {
        let Some(center) = position_of(row, scales)
        else {
            continue;
        };
        let geometry = match (x, y) {
            (Some(Scale::Band(_) | Scale::Point(_)), Some(other)) if other.numeric_domain().is_some() => {
                MarkGeometry::Segment { from: Point::new(center.x, center.y - half), to: Point::new(center.x, center.y + half) }
            }
            (Some(other), Some(Scale::Band(_) | Scale::Point(_))) if other.numeric_domain().is_some() => {
                MarkGeometry::Segment { from: Point::new(center.x - half, center.y), to: Point::new(center.x + half, center.y) }
            }
            _ => {
                return Err(ChartError::new(
                    codes::INVALID_SPEC,
                    "the tick mark needs a categorical scale on one axis and a continuous scale on the other",
                ));
            }
        };
        marks.push(MarkInstance {
            id: format!("chart-tick-{index}"),
            mark: Mark::Tick,
            source: source_for(row),
            geometry,
            style: stroked_style(row, scales, paints, RULE_WIDTH),
        });
    }
    Ok(())
}

fn build_texts(rows: &[Row], scales: &[ChartScale], paints: PaintDefaults, marks: &mut Vec<MarkInstance>) {
    for (index, row) in rows.iter().enumerate() {
        let Some(anchor) = position_of(row, scales)
        else {
            continue;
        };
        let text = row.get(Channel::Y).map(Value::label).or_else(|| row.get(Channel::X).map(Value::label)).unwrap_or_default();
        marks.push(MarkInstance {
            id: format!("chart-text-{index}"),
            mark: Mark::Text,
            source: source_for(row),
            geometry: MarkGeometry::Label { anchor, text },
            style: MarkStyle { opacity: opacity_for(row, scales, paints), ..MarkStyle::filled(paints.text_color) },
        });
    }
}

fn build_axis(channel: Channel, scales: &[ChartScale], plot: &Rect, options: &ChartOptions) -> Option<AxisPlan> {
    let scale = find_scale(scales, channel)?;
    let (from, to) = match channel {
        Channel::X => (Point::new(plot.x, plot.bottom()), Point::new(plot.right(), plot.bottom())),
        Channel::Y => (Point::new(plot.x, plot.bottom()), Point::new(plot.x, plot.y)),
        _ => return None,
    };

    let mut ticks = Vec::new();
    match scale {
        Scale::Band(band) => {
            for (index, label) in band.domain.iter().enumerate() {
                ticks.push(make_tick(channel, band.center_at(index), label.clone(), plot, options));
            }
        }
        Scale::Point(point) => {
            for (index, label) in point.domain.iter().enumerate() {
                ticks.push(make_tick(channel, point.position_at(index), label.clone(), plot, options));
            }
        }
        Scale::Linear(linear) => {
            for value in linear_ticks(linear.domain, options.tick_count) {
                ticks.push(make_tick(channel, linear.map(value), format_number(value), plot, options));
            }
        }
        Scale::Transformed(transformed) => {
            let values = match transformed.transfer {
                Transfer::Log => log_ticks(transformed.domain, options.tick_count),
                Transfer::Sqrt => linear_ticks(transformed.domain, options.tick_count),
            };
            for value in values {
                ticks.push(make_tick(channel, transformed.map(value), format_number(value), plot, options));
            }
        }
        Scale::Nominal(_) => return None,
    }
    Some(AxisPlan { channel, from, to, ticks })
}

fn make_tick(channel: Channel, position: f64, label: String, plot: &Rect, options: &ChartOptions) -> AxisTick {
    match channel {
        Channel::X => AxisTick {
            label,
            position: Point::new(position, plot.bottom()),
            label_position: Point::new(position, plot.bottom() + options.font_size + AXIS_LABEL_GAP),
        },
        _ => AxisTick {
            label,
            position: Point::new(plot.x, position),
            label_position: Point::new(plot.x - AXIS_LABEL_INSET, position + options.font_size * 0.35),
        },
    }
}

#[derive(Clone, Debug, Default)]
struct Row {
    x: Option<Value>,
    y: Option<Value>,
    color: Option<Value>,
    size: Option<Value>,
    shape: Option<Value>,
    opacity: Option<Value>,
}

impl Row {
    fn get(&self, channel: Channel) -> Option<&Value> {
        match channel {
            Channel::X => self.x.as_ref(),
            Channel::Y => self.y.as_ref(),
            Channel::Color => self.color.as_ref(),
            Channel::Size => self.size.as_ref(),
            Channel::Shape => self.shape.as_ref(),
            Channel::Opacity => self.opacity.as_ref(),
        }
    }

    fn set(&mut self, channel: Channel, value: Value) {
        match channel {
            Channel::X => self.x = Some(value),
            Channel::Y => self.y = Some(value),
            Channel::Color => self.color = Some(value),
            Channel::Size => self.size = Some(value),
            Channel::Shape => self.shape = Some(value),
            Channel::Opacity => self.opacity = Some(value),
        }
    }

    fn keys(&self, measure: Option<Channel>) -> Vec<Value> {
        Channel::ALL.iter().filter(|channel| Some(**channel) != measure).filter_map(|channel| self.get(*channel).cloned()).collect()
    }
}

fn channel_index(channel: Channel) -> usize {
    match channel {
        Channel::X => 0,
        Channel::Y => 1,
        Channel::Color => 2,
        Channel::Size => 3,
        Channel::Shape => 4,
        Channel::Opacity => 5,
    }
}
