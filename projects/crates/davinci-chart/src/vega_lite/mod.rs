//! Strict Vega-Lite subset lowered onto the chart domain.
//!
//! Vega-Lite is a JSON document, not a fence grammar, so this module is a
//! `serde` model rather than a parser. It accepts inline `data.values`, one
//! mark from `bar` / `line` / `area` / `point` / `rect` / `tick`, the `x` /
//! `y` / `color` / `size` / `opacity` channels, and the `linear` / `log` /
//! `sqrt` / `band` / `point` / `ordinal` scales the chart kernel can train.
//! Linear scales additionally accept the opt-in `zero` and `nice` domain
//! options.
//!
//! Everything else is refused before lowering with a `davinci.chart.*`
//! diagnostic that names the JSON path. No JavaScript runtime, no evaluation
//! of `transform`, and no second chart kernel: the result is a [`ChartSpec`]
//! plus a [`DataSnapshot`] for [`crate::compile`].

use davinci_scene::{Color, SemanticScene};
use davinci_types::{Diagnostic, DiagnosticSeverity};
use serde_json::{Map, Value as Json};

use crate::{
    Aggregate, Channel, ChannelEncoding, ChartCompilePlan, ChartError, ChartOptions, ChartSpec, DataColumn, DataSnapshot, FieldKind, FieldRef,
    Guide, Mark, ScaleSpec,
    error::codes,
};

/// A lowered Vega-Lite document ready for rendering.
#[derive(Clone, Debug, PartialEq)]
pub struct VegaChart {
    /// The authored chart title, when the document declares one.
    pub title: Option<String>,
    /// Chart semantics built from the document.
    pub spec: ChartSpec,
    /// Columnar snapshot built from `data.values`.
    pub data: DataSnapshot,
    /// Canvas options built from `width` / `height`.
    pub options: ChartOptions,
    /// The deterministic compile result.
    pub plan: ChartCompilePlan,
    /// The semantic scene rendered from [`VegaChart::plan`].
    pub scene: SemanticScene,
    /// Non-fatal loss records from compilation.
    pub diagnostics: Vec<Diagnostic>,
}

impl VegaChart {
    /// Render the scene to SVG through the shared display pipeline.
    pub fn to_svg(&self) -> Result<String, davinci_scene::SceneError> {
        self.scene.resolve(&davinci_scene::SceneResolver::default()).to_svg()
    }

    /// Render the scene to SVG with page-level hooks.
    ///
    /// Every emitted `id` (axes, ticks, marks, clip paths) is prefixed with
    /// `{id_namespace}-` and every semantic node gains
    /// `class="{class_prefix}-{role}"`, so several charts can share one page
    /// without id collisions. Chart chrome follows the theme palette while
    /// mark colors stay authored by the compile plan.
    pub fn to_svg_with_hooks(&self, id_namespace: &str, class_prefix: &str) -> Result<String, davinci_scene::SceneError> {
        self.scene
            .resolve(&davinci_scene::SceneResolver::default())
            .to_svg_with_chart_hooks(id_namespace, class_prefix, &davinci_types::DIAGRAM_THEME)
    }
}

const CHART_TOP_KEYS: &[&str] = &["$schema", "title", "width", "height", "data", "mark", "encoding"];

const UNSUPPORTED_TOP_KEYS: &[(&str, &str)] = &[
    ("transform", codes::UNSUPPORTED_TRANSFORM),
    ("layer", codes::UNSUPPORTED_VIEW),
    ("hconcat", codes::UNSUPPORTED_VIEW),
    ("vconcat", codes::UNSUPPORTED_VIEW),
    ("facet", codes::UNSUPPORTED_VIEW),
    ("repeat", codes::UNSUPPORTED_VIEW),
    ("resolve", codes::UNSUPPORTED_VIEW),
    ("concat", codes::UNSUPPORTED_VIEW),
    ("spec", codes::UNSUPPORTED_VIEW),
    ("projection", codes::UNSUPPORTED_VIEW),
    ("params", codes::UNSUPPORTED_INTERACTION),
    ("selection", codes::UNSUPPORTED_INTERACTION),
    ("config", codes::UNSUPPORTED_VIEW),
];

/// Parse, validate, and lower one Vega-Lite document.
pub fn lower(source: &str) -> Result<VegaChart, ChartError> {
    let document: Json = serde_json::from_str(source).map_err(|error| ChartError::new(codes::INVALID_JSON, format!("at `$`: {error}")))?;
    let Some(top) = document.as_object()
    else {
        return Err(fail(codes::INVALID_SPEC, "$", "the chart spec must be a JSON object"));
    };

    let mut title = None;
    let mut width = None;
    let mut height = None;
    let mut mark = None;
    let mut data = None;
    let mut encoding = None;

    for (key, value) in top {
        if let Some((_, code)) = UNSUPPORTED_TOP_KEYS.iter().find(|(name, _)| name == key) {
            return Err(fail(*code, &format!("$.{key}"), "this construct is refused in the first chart subset"));
        }
        match key.as_str() {
            "$schema" => {
                if !value.is_string() {
                    return Err(fail(codes::INVALID_SPEC, "$.$schema", "`$schema` must be a string"));
                }
            }
            "title" => title = Some(string_at(value, "$.title")?.to_string()),
            "width" => width = Some(number_at(value, "$.width")?),
            "height" => height = Some(number_at(value, "$.height")?),
            "data" => data = Some(value),
            "mark" => mark = Some(value),
            "encoding" => encoding = Some(value),
            other => {
                debug_assert!(!CHART_TOP_KEYS.contains(&other));
                return Err(fail(codes::UNKNOWN_KEY, &format!("$.{other}"), "unknown top-level key"));
            }
        }
    }

    let lowered_mark = lower_mark(mark.ok_or_else(|| fail(codes::INVALID_SPEC, "$.mark", "a chart mark is required"))?)?;
    let rows = lower_data(data.ok_or_else(|| fail(codes::INVALID_SPEC, "$.data", "inline `data.values` is required"))?)?;
    let bindings = lower_encoding(encoding.ok_or_else(|| fail(codes::INVALID_SPEC, "$.encoding", "an encoding is required"))?)?;

    let mut fields: Vec<FieldRef> = Vec::new();
    for binding in &bindings {
        if let Some(existing) = fields.iter().find(|field| field.name == binding.name) {
            if existing.kind != binding.kind {
                return Err(fail(
                    codes::TYPE_MISMATCH,
                    &binding.path,
                    format!("field `{}` is declared both `{}` and `{}`", binding.name, existing.kind.as_str(), binding.kind.as_str()),
                ));
            }
            continue;
        }
        fields.push(FieldRef::new(binding.name.clone(), binding.kind));
    }

    let columns = fields
        .iter()
        .map(|field| {
            let values = rows
                .iter()
                .enumerate()
                .map(|(index, row)| cell_value(row, field, index))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(DataColumn::new(field.name.clone(), field.kind, values))
        })
        .collect::<Result<Vec<_>, ChartError>>()?;

    let mut spec = ChartSpec::new(lowered_mark.mark);
    if let Some(color) = lowered_mark.color {
        spec.mark_color = Some(color);
    }
    if let Some(opacity) = lowered_mark.opacity {
        spec.mark_opacity = Some(opacity);
    }
    spec.fields = fields;
    for binding in &bindings {
        spec.encoding.set(
            binding.channel,
            ChannelEncoding { field: FieldRef::new(binding.name.clone(), binding.kind), aggregate: binding.aggregate },
        );
        if let Some(scale) = binding.scale {
            spec.scales.set(binding.channel, scale);
        }
    }
    let mut guides = Vec::new();
    if spec.encoding.x.is_some() {
        guides.push(Guide::axis(Channel::X));
    }
    if spec.encoding.y.is_some() {
        guides.push(Guide::axis(Channel::Y));
    }
    spec.guides = guides;

    let mut options = ChartOptions::default();
    if let Some(width) = width {
        options.width = positive(width, "$.width")?;
    }
    if let Some(height) = height {
        options.height = positive(height, "$.height")?;
    }

    let data = DataSnapshot::new(columns);
    for diagnostic in data.validate().into_iter().chain(spec.validate()) {
        if diagnostic.severity() == DiagnosticSeverity::Error {
            return Err(ChartError::from_diagnostic(&diagnostic));
        }
    }

    let lowered = crate::lower(&spec, &data, &options)?;
    Ok(VegaChart {
        title,
        spec,
        data,
        options,
        plan: lowered.plan,
        scene: lowered.scene,
        diagnostics: lowered.diagnostics,
    })
}

/// One channel binding produced by [`lower_encoding`].
#[derive(Clone, Debug, PartialEq)]
struct Binding {
    /// The bound channel.
    channel: Channel,
    /// The field name.
    name: String,
    /// The declared field kind.
    kind: FieldKind,
    /// The requested statistic.
    aggregate: Option<Aggregate>,
    /// The declared scale, if any.
    scale: Option<ScaleSpec>,
    /// The JSON path used by diagnostics.
    path: String,
}

struct LoweredMark {
    mark: Mark,
    color: Option<Color>,
    opacity: Option<f64>,
}

fn lower_mark(value: &Json) -> Result<LoweredMark, ChartError> {
    let (keyword, path, color, opacity) = match value {
        Json::String(text) => (text.as_str(), "$.mark".to_string(), None, None),
        Json::Object(object) => {
            let mut color = None;
            let mut opacity = None;
            for (key, item) in object {
                match key.as_str() {
                    "type" => {
                        if !item.is_string() {
                            return Err(fail(codes::INVALID_SPEC, "$.mark.type", "`mark.type` must be a string"));
                        }
                    }
                    "color" => color = Some(parse_color(item, "$.mark.color")?),
                    "opacity" => opacity = Some(parse_opacity(item, "$.mark.opacity")?),
                    other => return Err(fail(codes::UNKNOWN_KEY, &format!("$.mark.{other}"), "unknown mark key")),
                }
            }
            let Some(Json::String(text)) = object.get("type")
            else {
                return Err(fail(codes::INVALID_SPEC, "$.mark.type", "`mark.type` is required"));
            };
            (text.as_str(), "$.mark.type".to_string(), color, opacity)
        }
        _ => return Err(fail(codes::INVALID_SPEC, "$.mark", "`mark` must be a string or an object")),
    };

    let mark = match keyword {
        "bar" => Mark::Bar,
        "line" => Mark::Line,
        "area" => Mark::Area,
        "point" => Mark::Point,
        "rect" => Mark::Rect,
        "tick" => Mark::Tick,
        other => {
            return Err(fail(
                codes::UNSUPPORTED_MARK,
                &path,
                format!("mark `{other}` is outside the first subset (`bar`, `line`, `area`, `point`, `rect`, `tick`)"),
            ))
        }
    };
    Ok(LoweredMark { mark, color, opacity })
}

fn parse_color(value: &Json, path: &str) -> Result<Color, ChartError> {
    let Some(text) = value.as_str()
    else {
        return Err(fail(codes::INVALID_SPEC, path, "`mark.color` must be a string"));
    };
    let Some(digits) = text.strip_prefix('#')
    else {
        return Err(fail(codes::UNSUPPORTED_ENCODING, path, "only `#rgb` and `#rrggbb` colors are supported"));
    };
    let expanded = match digits.len() {
        3 => digits.chars().flat_map(|digit| [digit, digit]).collect::<String>(),
        6 => digits.to_string(),
        _ => return Err(fail(codes::UNSUPPORTED_ENCODING, path, "only `#rgb` and `#rrggbb` colors are supported")),
    };
    let parse = |slice: &str| u8::from_str_radix(slice, 16).map_err(|_| fail(codes::UNSUPPORTED_ENCODING, path, "color digits must be hexadecimal"));
    Ok(Color::rgb(parse(&expanded[0..2])?, parse(&expanded[2..4])?, parse(&expanded[4..6])?))
}

fn parse_opacity(value: &Json, path: &str) -> Result<f64, ChartError> {
    let Some(number) = value.as_f64()
    else {
        return Err(fail(codes::INVALID_SPEC, path, "`mark.opacity` must be a number"));
    };
    if !number.is_finite() || !(0.0..=1.0).contains(&number) {
        return Err(fail(codes::INVALID_SPEC, path, "`mark.opacity` must be within 0.0..=1.0"));
    }
    Ok(number)
}

fn lower_data(value: &Json) -> Result<Vec<Map<String, Json>>, ChartError> {
    let Some(object) = value.as_object()
    else {
        return Err(fail(codes::INVALID_SPEC, "$.data", "`data` must be an object"));
    };
    let mut rows = None;
    for (key, item) in object {
        match key.as_str() {
            "values" => {
                let Some(array) = item.as_array()
                else {
                    return Err(fail(codes::INVALID_SPEC, "$.data.values", "`data.values` must be an array of objects"));
                };
                let mut collected = Vec::with_capacity(array.len());
                for (index, row) in array.iter().enumerate() {
                    let Some(row) = row.as_object()
                    else {
                        return Err(fail(codes::INVALID_SPEC, &format!("$.data.values[{index}]"), "every row must be a JSON object"));
                    };
                    collected.push(row.clone());
                }
                rows = Some(collected);
            }
            "url" | "name" | "format" | "sequence" | "generator" => {
                return Err(fail(codes::REMOTE_DATA, &format!("$.data.{key}"), "only inline `data.values` is supported"));
            }
            other => return Err(fail(codes::UNKNOWN_KEY, &format!("$.data.{other}"), "unknown data key")),
        }
    }
    rows.ok_or_else(|| fail(codes::REMOTE_DATA, "$.data", "inline `data.values` is required"))
}

fn lower_encoding(value: &Json) -> Result<Vec<Binding>, ChartError> {
    let Some(object) = value.as_object()
    else {
        return Err(fail(codes::INVALID_SPEC, "$.encoding", "`encoding` must be an object"));
    };
    for key in object.keys() {
        match key.as_str() {
            "x" | "y" | "color" | "size" | "opacity" => {}
            "shape" | "detail" | "tooltip" | "order" | "text" | "longitude" | "latitude" | "href" | "url" => {
                return Err(fail(
                    codes::UNSUPPORTED_CHANNEL,
                    &format!("$.encoding.{key}"),
                    "channel is outside the first subset (`x`, `y`, `color`, `size`, `opacity`)",
                ));
            }
            other => return Err(fail(codes::UNKNOWN_KEY, &format!("$.encoding.{other}"), "unknown channel")),
        }
    }

    let mut bindings = Vec::new();
    for channel in Channel::ALL {
        let Some(item) = object.get(channel.as_str())
        else {
            continue;
        };
        bindings.push(lower_channel(channel, item, &format!("$.encoding.{}", channel.as_str()))?);
    }
    if bindings.is_empty() {
        return Err(fail(codes::INVALID_SPEC, "$.encoding", "at least one channel must be bound"));
    }
    Ok(bindings)
}

fn lower_channel(channel: Channel, value: &Json, path: &str) -> Result<Binding, ChartError> {
    let Some(object) = value.as_object()
    else {
        return Err(fail(codes::INVALID_SPEC, path, "a channel definition must be an object"));
    };

    let mut name = None;
    let mut kind = None;
    let mut aggregate = None;
    let mut scale = None;

    for (key, item) in object {
        match key.as_str() {
            "field" => name = Some(string_at(item, &format!("{path}.field"))?.to_string()),
            "type" => kind = Some(kind_at(item, &format!("{path}.type"))?),
            "aggregate" => aggregate = Some(aggregate_at(item, &format!("{path}.aggregate"))?),
            "scale" => scale = Some(lower_scale(item, &format!("{path}.scale"))?),
            "axis" | "legend" | "sort" | "stack" | "condition" => {
                return Err(fail(
                    codes::UNSUPPORTED_ENCODING,
                    &format!("{path}.{key}"),
                    "channel options are declared by the chart domain, not by the document",
                ));
            }
            "bin" => {
                return Err(fail(codes::UNSUPPORTED_ENCODING, &format!("{path}.bin"), "binning is refused in the first subset"));
            }
            other => return Err(fail(codes::UNKNOWN_KEY, &format!("{path}.{other}"), "unknown channel key")),
        }
    }

    let name = name.ok_or_else(|| fail(codes::INVALID_SPEC, path, "`field` is required"))?;
    if name.is_empty() {
        return Err(fail(codes::INVALID_SPEC, &format!("{path}.field"), "`field` must not be empty"));
    }
    let kind = kind.ok_or_else(|| fail(codes::INVALID_SPEC, path, "`type` is required"))?;

    Ok(Binding { channel, name, kind, aggregate, scale, path: format!("{path}.field") })
}

fn lower_scale(value: &Json, path: &str) -> Result<ScaleSpec, ChartError> {
    let Some(object) = value.as_object()
    else {
        return Err(fail(codes::INVALID_SPEC, path, "`scale` must be an object"));
    };

    let mut scale_type = None;
    let mut domain = None;
    let mut padding = None;
    let mut zero = None;
    let mut nice = None;
    for (key, item) in object {
        match key.as_str() {
            "type" => scale_type = Some(string_at(item, &format!("{path}.type"))?.to_string()),
            "padding" => padding = Some(number_at(item, &format!("{path}.padding"))?),
            "zero" => zero = Some(boolean_at(item, &format!("{path}.zero"))?),
            "nice" => nice = Some(boolean_at(item, &format!("{path}.nice"))?),
            "domain" => {
                let Some(array) = item.as_array()
                else {
                    return Err(fail(codes::UNSUPPORTED_SCALE, &format!("{path}.domain"), "only a numeric `[min, max]` domain is supported"));
                };
                if array.len() != 2 {
                    return Err(fail(codes::UNSUPPORTED_SCALE, &format!("{path}.domain"), "the domain must have two bounds"));
                }
                let min = number_at(&array[0], &format!("{path}.domain[0]"))?;
                let max = number_at(&array[1], &format!("{path}.domain[1]"))?;
                domain = Some((min, max));
            }
            other => {
                return Err(fail(
                    codes::UNSUPPORTED_SCALE,
                    &format!("{path}.{other}"),
                    "this scale option is not compiled yet (`type`, `domain`, `padding`, `zero`, and `nice` are supported)",
                ));
            }
        }
    }

    let scale_type = scale_type.ok_or_else(|| fail(codes::INVALID_SPEC, path, "`scale.type` is required"))?;
    match scale_type.as_str() {
        "linear" => {
            if zero == Some(true) && domain.is_some() {
                return Err(fail(codes::INVALID_SCALE, &format!("{path}.zero"), "`zero` cannot widen an explicit `domain`"));
            }
            Ok(ScaleSpec::Linear { domain, zero: zero.unwrap_or(false), nice: nice.unwrap_or(false) })
        }
        "log" => {
            refuse_non_linear_options(path, zero, nice)?;
            Ok(ScaleSpec::Log { domain })
        }
        "sqrt" => {
            refuse_non_linear_options(path, zero, nice)?;
            Ok(ScaleSpec::Sqrt { domain })
        }
        "band" => {
            refuse_non_linear_options(path, zero, nice)?;
            if domain.is_some() {
                return Err(fail(codes::INVALID_SCALE, &format!("{path}.domain"), "a band scale cannot take a numeric domain"));
            }
            Ok(ScaleSpec::Band { padding: padding.unwrap_or(0.1) })
        }
        "point" => {
            refuse_non_linear_options(path, zero, nice)?;
            if domain.is_some() {
                return Err(fail(codes::INVALID_SCALE, &format!("{path}.domain"), "a point scale cannot take a numeric domain"));
            }
            Ok(ScaleSpec::Point { padding: padding.unwrap_or(0.5) })
        }
        "ordinal" => {
            refuse_non_linear_options(path, zero, nice)?;
            Ok(ScaleSpec::Nominal)
        }
        other => Err(fail(codes::UNSUPPORTED_SCALE, &format!("{path}.type"), format!("the chart kernel cannot train a `{other}` scale yet"))),
    }
}

fn refuse_non_linear_options(path: &str, zero: Option<bool>, nice: Option<bool>) -> Result<(), ChartError> {
    for (option, present) in [("zero", zero.is_some()), ("nice", nice.is_some())] {
        if present {
            return Err(fail(
                codes::UNSUPPORTED_SCALE,
                &format!("{path}.{option}"),
                format!("`{option}` only applies to linear scales in this slice"),
            ));
        }
    }
    Ok(())
}

fn cell_value(row: &Map<String, Json>, field: &FieldRef, index: usize) -> Result<crate::Value, ChartError> {
    let path = format!("$.data.values[{index}].{}", field.name);
    let Some(cell) = row.get(&field.name)
    else {
        return Ok(crate::Value::Null);
    };

    match (field.kind, cell) {
        (_, Json::Null) => Ok(crate::Value::Null),
        (FieldKind::Quantitative, Json::Number(number)) => match number.as_f64() {
            Some(value) if value.is_finite() => Ok(crate::Value::Number(value)),
            _ => Err(fail(codes::NON_FINITE_VALUE, &path, "quantitative values must be finite")),
        },
        (FieldKind::Temporal, Json::Number(number)) => match number.as_i64() {
            Some(millis) => Ok(crate::Value::Timestamp(millis)),
            None => Err(fail(codes::TYPE_MISMATCH, &path, "temporal values must be epoch milliseconds")),
        },
        (FieldKind::Nominal | FieldKind::Ordinal, Json::String(text)) => Ok(crate::Value::Text(text.clone())),
        (FieldKind::Nominal | FieldKind::Ordinal, Json::Number(number)) => Ok(crate::Value::Text(number.to_string())),
        (kind, _) => Err(fail(
            codes::TYPE_MISMATCH,
            &path,
            format!("a `{}` field cannot hold a {} value", kind.as_str(), json_kind(cell)),
        )),
    }
}

fn kind_at(value: &Json, path: &str) -> Result<FieldKind, ChartError> {
    let keyword = string_at(value, path)?;
    match keyword {
        "quantitative" => Ok(FieldKind::Quantitative),
        "ordinal" => Ok(FieldKind::Ordinal),
        "nominal" => Ok(FieldKind::Nominal),
        "temporal" => Ok(FieldKind::Temporal),
        other => Err(fail(
            codes::UNSUPPORTED_ENCODING,
            path,
            format!("field type `{other}` is outside the subset (`quantitative`, `ordinal`, `nominal`, `temporal`)"),
        )),
    }
}

fn aggregate_at(value: &Json, path: &str) -> Result<Aggregate, ChartError> {
    let keyword = string_at(value, path)?;
    match keyword {
        "count" => Ok(Aggregate::Count),
        "sum" => Ok(Aggregate::Sum),
        "mean" => Ok(Aggregate::Mean),
        "min" => Ok(Aggregate::Min),
        "max" => Ok(Aggregate::Max),
        other => Err(fail(
            codes::UNSUPPORTED_ENCODING,
            path,
            format!("aggregate `{other}` is outside the subset (`count`, `sum`, `mean`, `min`, `max`)"),
        )),
    }
}

fn string_at<'a>(value: &'a Json, path: &str) -> Result<&'a str, ChartError> {
    value.as_str().ok_or_else(|| fail(codes::INVALID_SPEC, path, "expected a string"))
}

fn number_at(value: &Json, path: &str) -> Result<f64, ChartError> {
    let number = value.as_f64().ok_or_else(|| fail(codes::INVALID_SPEC, path, "expected a number"))?;
    if !number.is_finite() {
        return Err(fail(codes::NON_FINITE_VALUE, path, "expected a finite number"));
    }
    Ok(number)
}

fn boolean_at(value: &Json, path: &str) -> Result<bool, ChartError> {
    value.as_bool().ok_or_else(|| fail(codes::INVALID_SPEC, path, "expected a boolean"))
}

fn positive(value: f64, path: &str) -> Result<f64, ChartError> {
    if value > 0.0 {
        Ok(value)
    }
    else {
        Err(fail(codes::INVALID_SPEC, path, "expected a positive number"))
    }
}

fn json_kind(value: &Json) -> &'static str {
    match value {
        Json::Null => "null",
        Json::Bool(_) => "boolean",
        Json::Number(_) => "number",
        Json::String(_) => "string",
        Json::Array(_) => "array",
        Json::Object(_) => "object",
    }
}

fn fail(code: &str, path: &str, message: impl Into<String>) -> ChartError {
    ChartError::new(code, format!("at `{path}`: {}", message.into()))
}
