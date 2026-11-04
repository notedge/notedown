//! Deterministic SVG serialization and scene validation.

use std::fmt::Write as _;

use davinci_types::{DiagramTheme, Point, Rect, Size, ThemeTokens, Transform};

use crate::{
    error::SceneError,
    path::{PathCommand, PathData},
    scene::{DisplayScene, MarkerShape, NodeKind, SceneNode, SemanticRole, ShapeStyle, TextStyle},
};

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

pub(crate) fn validate_scene(scene: &DisplayScene) -> Result<(), SceneError> {
    validate_extent(scene.size, "scene.size")?;
    for (index, node) in scene.nodes.iter().enumerate() {
        validate_node(node, &format!("scene.nodes[{index}]"))?;
    }
    Ok(())
}

fn validate_node(node: &SceneNode, context: &str) -> Result<(), SceneError> {
    validate_transform(node.transform, &format!("{context}.transform"))?;
    if let Some(clip) = node.clip {
        validate_rect(clip, &format!("{context}.clip"))?;
    }
    if let Some(hit_region) = node.hit_region {
        validate_rect(hit_region, &format!("{context}.hit_region"))?;
    }

    match &node.kind {
        NodeKind::Group { children, opacity } => {
            validate_opacity(*opacity, &format!("{context}.opacity"))?;
            for (index, child) in children.iter().enumerate() {
                validate_node(child, &format!("{context}.children[{index}]"))?;
            }
        }
        NodeKind::Path { path, style } => {
            validate_path(path, &format!("{context}.path"))?;
            validate_shape_style(style, &format!("{context}.style"))?;
        }
        NodeKind::Line { from, to, style } => {
            validate_point(*from, &format!("{context}.from"))?;
            validate_point(*to, &format!("{context}.to"))?;
            validate_shape_style(style, &format!("{context}.style"))?;
        }
        NodeKind::Rect { rect, style } => {
            validate_rect(*rect, &format!("{context}.rect"))?;
            validate_shape_style(style, &format!("{context}.style"))?;
        }
        NodeKind::RoundedRect { rect, radius, style } => {
            validate_rect(*rect, &format!("{context}.rect"))?;
            validate_non_negative(*radius, &format!("{context}.radius"))?;
            validate_shape_style(style, &format!("{context}.style"))?;
        }
        NodeKind::Ellipse { center, radii, style } => {
            validate_point(*center, &format!("{context}.center"))?;
            validate_extent(*radii, &format!("{context}.radii"))?;
            validate_shape_style(style, &format!("{context}.style"))?;
        }
        NodeKind::Text { position, style, .. } => {
            validate_point(*position, &format!("{context}.position"))?;
            validate_text_style(style, &format!("{context}.style"))?;
        }
        NodeKind::Image { rect, opacity, .. } => {
            validate_rect(*rect, &format!("{context}.rect"))?;
            validate_opacity(*opacity, &format!("{context}.opacity"))?;
        }
        NodeKind::Marker { at, size, style, .. } => {
            validate_point(*at, &format!("{context}.at"))?;
            validate_non_negative(*size, &format!("{context}.size"))?;
            validate_shape_style(style, &format!("{context}.style"))?;
        }
    }
    Ok(())
}

fn validate_shape_style(style: &ShapeStyle, context: &str) -> Result<(), SceneError> {
    validate_opacity(style.opacity, &format!("{context}.opacity"))?;
    validate_non_negative(style.stroke_width, &format!("{context}.stroke_width"))
}

fn validate_text_style(style: &TextStyle, context: &str) -> Result<(), SceneError> {
    validate_opacity(style.opacity, &format!("{context}.opacity"))?;
    validate_non_negative(style.font_size, &format!("{context}.font_size"))?;
    if style.font_size == 0.0 {
        return Err(SceneError::invalid_style(format!("{context}.font_size must be positive")));
    }
    Ok(())
}

fn validate_path(path: &PathData, context: &str) -> Result<(), SceneError> {
    for (index, command) in path.commands.iter().enumerate() {
        let command_context = format!("{context}.commands[{index}]");
        match *command {
            PathCommand::MoveTo(point) | PathCommand::LineTo(point) => validate_point(point, &command_context)?,
            PathCommand::QuadraticTo { control, to } => {
                validate_point(control, &format!("{command_context}.control"))?;
                validate_point(to, &format!("{command_context}.to"))?;
            }
            PathCommand::CubicTo { control1, control2, to } => {
                validate_point(control1, &format!("{command_context}.control1"))?;
                validate_point(control2, &format!("{command_context}.control2"))?;
                validate_point(to, &format!("{command_context}.to"))?;
            }
            PathCommand::Close => {}
        }
    }
    Ok(())
}

fn validate_point(point: Point, context: &str) -> Result<(), SceneError> {
    validate_finite(point.x, &format!("{context}.x"))?;
    validate_finite(point.y, &format!("{context}.y"))
}

fn validate_extent(size: Size, context: &str) -> Result<(), SceneError> {
    validate_non_negative(size.width, &format!("{context}.width"))?;
    validate_non_negative(size.height, &format!("{context}.height"))
}

fn validate_rect(rect: Rect, context: &str) -> Result<(), SceneError> {
    validate_finite(rect.x, &format!("{context}.x"))?;
    validate_finite(rect.y, &format!("{context}.y"))?;
    validate_non_negative(rect.width, &format!("{context}.width"))?;
    validate_non_negative(rect.height, &format!("{context}.height"))
}

fn validate_transform(transform: Transform, context: &str) -> Result<(), SceneError> {
    for (name, value) in
        [("a", transform.a()), ("b", transform.b()), ("c", transform.c()), ("d", transform.d()), ("e", transform.e()), ("f", transform.f())]
    {
        if !value.is_finite() {
            return Err(SceneError::invalid_transform(format!("{context}.{name} must be finite, found {value}")));
        }
    }
    Ok(())
}

fn validate_opacity(value: f64, context: &str) -> Result<(), SceneError> {
    validate_finite(value, context)?;
    if !(0.0..=1.0).contains(&value) {
        return Err(SceneError::invalid_style(format!("{context} must be within 0..=1, found {value}")));
    }
    Ok(())
}

fn validate_non_negative(value: f64, context: &str) -> Result<(), SceneError> {
    validate_finite(value, context)?;
    if value < 0.0 {
        return Err(SceneError::invalid_geometry(format!("{context} must not be negative, found {value}")));
    }
    Ok(())
}

fn validate_finite(value: f64, context: &str) -> Result<(), SceneError> {
    if value.is_finite() { Ok(()) } else { Err(SceneError::invalid_geometry(format!("{context} must be finite, found {value}"))) }
}

// ---------------------------------------------------------------------------
// Writer
// ---------------------------------------------------------------------------

/// How a node takes its paint when page hooks are active.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PaintMode {
    /// No hooks: paint is emitted as presentation attributes.
    Inline,
    /// Paint comes only from the embedded palette.
    Themed,
    /// Paint is authored per instance and emitted as an inline `style`,
    /// which outranks the palette rule.
    Authored,
}

/// Which semantic roles a palette owns.
///
/// Diagrams tint every role from theme tokens. Charts only own their chrome,
/// because a mark's color is data semantics coming from the encoding or the
/// mark declaration and must survive theming.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RolePalette {
    /// Every role is theme-owned.
    Diagram,
    /// Only `axis` and `tick` are theme-owned, marks keep authored paint.
    Chart,
}

impl RolePalette {
    fn mode(self, role: Option<SemanticRole>, inherited: PaintMode) -> PaintMode {
        let Some(role) = role else {
            return inherited;
        };
        match self {
            Self::Diagram => PaintMode::Themed,
            Self::Chart => match role {
                SemanticRole::Mark | SemanticRole::Label => PaintMode::Authored,
                _ => PaintMode::Themed,
            },
        }
    }
}

/// Page-level naming hooks for SVG serialization: a per-scene id namespace,
/// a shared class prefix for semantic roles, and the active palette.
pub(crate) struct SvgHooks<'a> {
    /// Prefix applied to every emitted `id` as `{id_namespace}-{id}`.
    pub id_namespace: &'a str,
    /// Prefix for role classes: `class="{class_prefix}-{role}"` plus
    /// `class="{class_prefix}"` on the root `<svg>`.
    pub class_prefix: &'a str,
    /// Theme tokens inlined as an embedded `<style>` block.
    pub theme: &'a DiagramTheme,
    /// Which roles the palette owns.
    pub palette: RolePalette,
}

pub(crate) fn write_svg(scene: &DisplayScene, hooks: Option<SvgHooks>) -> Result<String, SceneError> {
    validate_scene(scene)?;

    let mut out = String::new();
    out.push_str("<svg xmlns=\"http://www.w3.org/2000/svg\"");
    if let Some(hooks) = &hooks {
        push_string_attribute(&mut out, "class", hooks.class_prefix);
    }
    push_number_attribute(&mut out, "width", scene.size.width);
    push_number_attribute(&mut out, "height", scene.size.height);
    out.push_str(" viewBox=\"0 0 ");
    out.push_str(&number(scene.size.width));
    out.push(' ');
    out.push_str(&number(scene.size.height));
    out.push_str("\">\n");
    let initial = match &hooks {
        Some(_) => PaintMode::Themed,
        None => PaintMode::Inline,
    };
    if let Some(hooks) = &hooks {
        write_theme_style(&mut out, hooks);
    }

    let mut clip_index = 0usize;
    for node in &scene.nodes {
        write_node(&mut out, node, 1, &mut clip_index, hooks.as_ref(), initial);
    }

    out.push_str("</svg>\n");
    Ok(out)
}

fn write_node(out: &mut String, node: &SceneNode, depth: usize, clip_index: &mut usize, hooks: Option<&SvgHooks>, kind: PaintMode) {
    match node.clip {
        Some(clip) => {
            let id = match hooks {
                Some(hooks) => format!("{}-clip-{clip_index}", hooks.id_namespace),
                None => format!("clip-{clip_index}"),
            };
            *clip_index += 1;

            let indent = indent(depth);
            out.push_str(&indent);
            out.push_str("<clipPath id=\"");
            out.push_str(&escape(&id));
            out.push_str("\">\n");
            out.push_str(&indent);
            out.push_str("  <rect");
            push_number_attribute(out, "x", clip.x);
            push_number_attribute(out, "y", clip.y);
            push_number_attribute(out, "width", clip.width);
            push_number_attribute(out, "height", clip.height);
            push_transform_attribute(out, node.transform);
            out.push_str("/>\n");
            out.push_str(&indent);
            out.push_str("</clipPath>\n");

            out.push_str(&indent);
            out.push_str("<g clip-path=\"url(#");
            out.push_str(&escape(&id));
            out.push_str(")\">\n");
            write_element(out, node, depth + 1, clip_index, hooks, kind);
            out.push_str(&indent);
            out.push_str("</g>\n");
        }
        None => write_element(out, node, depth, clip_index, hooks, kind),
    }
}

fn write_element(out: &mut String, node: &SceneNode, depth: usize, clip_index: &mut usize, hooks: Option<&SvgHooks>, inherited: PaintMode) {
    let indent = indent(depth);
    // A role-bearing element takes the paint mode its palette assigns. A
    // role-less node inherits its parent's mode, so a decoration inside a
    // themed group stays themed.
    let paint = match hooks {
        Some(hooks) => hooks.palette.mode(node.role, inherited),
        None => PaintMode::Inline,
    };
    let child_paint = paint;
    match &node.kind {
        NodeKind::Group { children, opacity } => {
            out.push_str(&indent);
            out.push_str("<g");
            push_id_attribute(out, node.id.as_deref(), hooks);
            push_semantic_attributes(out, node, hooks);
            push_transform_attribute(out, node.transform);
            if *opacity != 1.0 {
                push_number_attribute(out, "opacity", *opacity);
            }
            out.push_str(">\n");
            for child in children {
                write_node(out, child, depth + 1, clip_index, hooks, child_paint);
            }
            out.push_str(&indent);
            out.push_str("</g>\n");
        }
        NodeKind::Path { path, style } => {
            out.push_str(&indent);
            out.push_str("<path");
            push_id_attribute(out, node.id.as_deref(), hooks);
            push_semantic_attributes(out, node, hooks);
            out.push_str(" d=\"");
            out.push_str(&path_data(path));
            out.push('"');
            push_transform_attribute(out, node.transform);
            write_shape_paint(out, style, true, paint);
            out.push_str("/>\n");
        }
        NodeKind::Line { from, to, style } => {
            out.push_str(&indent);
            out.push_str("<line");
            push_id_attribute(out, node.id.as_deref(), hooks);
            push_semantic_attributes(out, node, hooks);
            push_number_attribute(out, "x1", from.x);
            push_number_attribute(out, "y1", from.y);
            push_number_attribute(out, "x2", to.x);
            push_number_attribute(out, "y2", to.y);
            push_transform_attribute(out, node.transform);
            write_shape_paint(out, style, false, paint);
            out.push_str("/>\n");
        }
        NodeKind::Rect { rect, style } => {
            out.push_str(&indent);
            out.push_str("<rect");
            push_id_attribute(out, node.id.as_deref(), hooks);
            push_semantic_attributes(out, node, hooks);
            push_rect_attributes(out, *rect);
            push_transform_attribute(out, node.transform);
            write_shape_paint(out, style, true, paint);
            out.push_str("/>\n");
        }
        NodeKind::RoundedRect { rect, radius, style } => {
            out.push_str(&indent);
            out.push_str("<rect");
            push_id_attribute(out, node.id.as_deref(), hooks);
            push_semantic_attributes(out, node, hooks);
            push_rect_attributes(out, *rect);
            push_number_attribute(out, "rx", *radius);
            push_transform_attribute(out, node.transform);
            write_shape_paint(out, style, true, paint);
            out.push_str("/>\n");
        }
        NodeKind::Ellipse { center, radii, style } => {
            out.push_str(&indent);
            out.push_str("<ellipse");
            push_id_attribute(out, node.id.as_deref(), hooks);
            push_semantic_attributes(out, node, hooks);
            push_number_attribute(out, "cx", center.x);
            push_number_attribute(out, "cy", center.y);
            push_number_attribute(out, "rx", radii.width);
            push_number_attribute(out, "ry", radii.height);
            push_transform_attribute(out, node.transform);
            write_shape_paint(out, style, true, paint);
            out.push_str("/>\n");
        }
        NodeKind::Text { position, text, style } => {
            out.push_str(&indent);
            out.push_str("<text");
            push_id_attribute(out, node.id.as_deref(), hooks);
            push_semantic_attributes(out, node, hooks);
            push_number_attribute(out, "x", position.x);
            push_number_attribute(out, "y", position.y);
            push_transform_attribute(out, node.transform);
            write_text_paint(out, style, paint);
            out.push('>');
            out.push_str(&escape(text));
            out.push_str("</text>\n");
        }
        NodeKind::Image { rect, resource, opacity } => {
            out.push_str(&indent);
            out.push_str("<image");
            push_id_attribute(out, node.id.as_deref(), hooks);
            push_semantic_attributes(out, node, hooks);
            push_rect_attributes(out, *rect);
            push_string_attribute(out, "href", &resource.href);
            if let Some(mime) = resource.mime.as_deref() {
                push_string_attribute(out, "data-mime", mime);
            }
            push_transform_attribute(out, node.transform);
            if *opacity != 1.0 {
                push_number_attribute(out, "opacity", *opacity);
            }
            out.push_str("/>\n");
        }
        NodeKind::Marker { at, shape, size, style } => {
            let half = size * 0.5;
            match shape {
                MarkerShape::Circle => {
                    out.push_str(&indent);
                    out.push_str("<circle");
                    push_id_attribute(out, node.id.as_deref(), hooks);
                    push_semantic_attributes(out, node, hooks);
                    push_number_attribute(out, "cx", at.x);
                    push_number_attribute(out, "cy", at.y);
                    push_number_attribute(out, "r", half);
                }
                MarkerShape::Triangle | MarkerShape::Diamond => {
                    out.push_str(&indent);
                    out.push_str("<polygon");
                    push_id_attribute(out, node.id.as_deref(), hooks);
                    push_semantic_attributes(out, node, hooks);
                    out.push_str(" points=\"");
                    let points = if matches!(*shape, MarkerShape::Triangle) {
                        vec![Point::new(at.x, at.y - half), Point::new(at.x - half, at.y + half), Point::new(at.x + half, at.y + half)]
                    }
                    else {
                        vec![
                            Point::new(at.x, at.y - half),
                            Point::new(at.x + half, at.y),
                            Point::new(at.x, at.y + half),
                            Point::new(at.x - half, at.y),
                        ]
                    };
                    for (index, point) in points.iter().enumerate() {
                        if index > 0 {
                            out.push(' ');
                        }
                        let _ = write!(out, "{},{}", number(point.x), number(point.y));
                    }
                    out.push('"');
                }
            }
            push_transform_attribute(out, node.transform);
            write_shape_paint(out, style, true, paint);
            out.push_str("/>\n");
        }
    }
}

fn push_semantic_attributes(out: &mut String, node: &SceneNode, hooks: Option<&SvgHooks>) {
    if let Some(role) = node.role {
        push_string_attribute(out, "data-role", role.as_str());
        if let Some(hooks) = hooks {
            push_string_attribute(out, "class", &format!("{}-{}", hooks.class_prefix, role.as_str()));
        }
    }
    if let Some(source) = node.source.as_ref() {
        if let Some(element) = source.element.as_deref() {
            push_string_attribute(out, "data-source", element);
        }
        if let Some((start, end)) = source.span {
            let _ = write!(out, " data-source-range=\"{start}-{end}\"");
        }
    }
}

/// Embed the theme as an inline `<style>` block: the light palette applies
/// by default and the dark palette behind `prefers-color-scheme: dark`.
/// Every selector hangs under the container class, so nothing leaks to the
/// host page. Geometry (stroke width, opacity) stays inline; only colors and
/// the font family come from the theme.
fn write_theme_style(out: &mut String, hooks: &SvgHooks) {
    out.push_str("<style>");
    write_palette(out, hooks.class_prefix, &hooks.theme.light, hooks.palette);
    out.push_str("@media (prefers-color-scheme: dark) {");
    write_palette(out, hooks.class_prefix, &hooks.theme.dark, hooks.palette);
    out.push_str("}");
    out.push_str("</style>\n");
}

fn write_palette(out: &mut String, prefix: &str, tokens: &ThemeTokens, palette: RolePalette) {
    let node_fill = tokens.node_fill.to_css();
    let node_stroke = tokens.node_stroke.to_css();
    let node_text = tokens.node_text.to_css();
    let edge_stroke = tokens.edge_stroke.to_css();
    let edge_label_text = tokens.edge_label_text.to_css();
    let edge_label_bg = tokens.edge_label_bg.to_css();
    let cluster_fill = tokens.cluster_fill.to_css();
    let diagram = format!(".{prefix} .{prefix}");
    let _ = write!(out, ".{prefix} .{prefix}-node {{ fill: {node_fill}; stroke: {node_stroke} }}");
    let _ = write!(out, ".{prefix} .{prefix}-cluster {{ fill: {cluster_fill}; stroke: {node_stroke} }}");
    let _ = write!(out, ".{prefix} .{prefix}-edge {{ fill: none; stroke: {edge_stroke} }}");
    let _ = write!(out, ".{prefix} .{prefix}-edge polygon, .{prefix} .{prefix}-edge circle {{ fill: {edge_stroke}; stroke: none }}");
    let _ = write!(out, ".{prefix} .{prefix}-edge-label-bg {{ fill: {edge_label_bg}; fill-opacity: {} }}", davinci_types::EDGE_LABEL_BG_OPACITY);
    let _ = write!(out, ".{prefix} .{prefix}-edge-label {{ fill: {edge_label_text} }}");
    let _ = write!(out, ".{prefix} .{prefix}-label {{ fill: {node_text} }}");
    let _ = write!(out, ".{prefix} .{prefix}-decoration {{ fill: none; stroke: {edge_stroke} }}");
    let _ = write!(out, ".{prefix} text {{ font-family: Trebuchet MS, Verdana, sans-serif }}");

    // Chart chrome. Marks carry authored colors as an inline `style`, so the
    // mark rule only supplies a neutral default and never repaints a datum.
    if palette == RolePalette::Chart {
        // Role classes sit on the painted element itself, so the selector must
        // qualify by tag: `line` for stroke-only chrome, `text` for labels.
        let _ = write!(out, ".{prefix} line.{prefix}-axis {{ stroke: {edge_stroke} }}");
        let _ = write!(out, ".{prefix} line.{prefix}-tick {{ stroke: {edge_stroke} }}");
        let _ = write!(out, ".{prefix} text.{prefix}-tick {{ fill: {node_text} }}");
        let _ = write!(out, "{diagram}-mark {{ fill: none; stroke: none }}");
        let _ = write!(out, "{diagram}-legend {{ stroke: {edge_stroke} }}");
    }
}

fn write_shape_paint(out: &mut String, style: &ShapeStyle, include_fill: bool, mode: PaintMode) {
    match mode {
        PaintMode::Inline => {
            if include_fill {
                match style.fill {
                    Some(color) => push_string_attribute(out, "fill", &color.to_css()),
                    None => push_string_attribute(out, "fill", "none"),
                }
            }
            if let Some(stroke) = style.stroke {
                push_string_attribute(out, "stroke", &stroke.to_css());
            }
            if style.opacity != 1.0 {
                push_number_attribute(out, "opacity", style.opacity);
            }
        }
        PaintMode::Themed => {}
        PaintMode::Authored => {
            let mut inline = String::new();
            // Authored marks declare both paint slots so the palette rule
            // cannot fill a stroked series or outline a filled bar.
            let _ = write!(inline, "fill: {};", if include_fill { style.fill.map_or("none".to_string(), |color| color.to_css()) } else { "none".to_string() });
            let _ = write!(inline, "stroke: {};", style.stroke.map_or("none".to_string(), |color| color.to_css()));
            if style.opacity != 1.0 {
                let _ = write!(inline, "opacity: {};", number(style.opacity));
            }
            push_string_attribute(out, "style", inline.trim_end_matches(';'));
        }
    }
    if let Some(_) = style.stroke {
        push_number_attribute(out, "stroke-width", style.stroke_width);
    }
}

fn write_text_paint(out: &mut String, style: &TextStyle, mode: PaintMode) {
    if mode != PaintMode::Themed {
        if let Some(family) = &style.font_family {
            push_string_attribute(out, "font-family", family);
        }
    }
    push_number_attribute(out, "font-size", style.font_size);
    if let Some(weight) = style.font_weight {
        out.push_str(" font-weight=\"");
        let _ = write!(out, "{weight}");
        out.push('"');
    }
    push_string_attribute(out, "text-anchor", style.text_anchor.as_svg());
    match mode {
        PaintMode::Inline => {
            if let Some(fill) = style.fill {
                push_string_attribute(out, "fill", &fill.to_css());
            }
            if style.opacity != 1.0 {
                push_number_attribute(out, "opacity", style.opacity);
            }
        }
        PaintMode::Themed => {}
        PaintMode::Authored => {
            let mut inline = String::new();
            let _ = write!(inline, "fill: {};", style.fill.map_or("none".to_string(), |fill| fill.to_css()));
            if style.opacity != 1.0 {
                let _ = write!(inline, "opacity: {};", number(style.opacity));
            }
            push_string_attribute(out, "style", inline.trim_end_matches(';'));
        }
    }
}

fn push_rect_attributes(out: &mut String, rect: Rect) {
    push_number_attribute(out, "x", rect.x);
    push_number_attribute(out, "y", rect.y);
    push_number_attribute(out, "width", rect.width);
    push_number_attribute(out, "height", rect.height);
}

fn push_id_attribute(out: &mut String, id: Option<&str>, hooks: Option<&SvgHooks>) {
    if let Some(id) = id {
        match hooks {
            Some(hooks) => push_string_attribute(out, "id", &format!("{}-{id}", hooks.id_namespace)),
            None => push_string_attribute(out, "id", id),
        }
    }
}

fn push_transform_attribute(out: &mut String, transform: Transform) {
    if transform.is_identity() {
        return;
    }
    out.push_str(" transform=\"matrix(");
    for (index, value) in [transform.a(), transform.b(), transform.c(), transform.d(), transform.e(), transform.f()].iter().enumerate() {
        if index > 0 {
            out.push_str(", ");
        }
        out.push_str(&number(*value));
    }
    out.push_str(")\"");
}

fn push_string_attribute(out: &mut String, name: &str, value: &str) {
    out.push(' ');
    out.push_str(name);
    out.push_str("=\"");
    out.push_str(&escape(value));
    out.push('"');
}

fn push_number_attribute(out: &mut String, name: &str, value: f64) {
    out.push(' ');
    out.push_str(name);
    out.push_str("=\"");
    out.push_str(&number(value));
    out.push('"');
}

fn path_data(path: &PathData) -> String {
    let mut data = String::new();
    for (index, command) in path.commands.iter().enumerate() {
        if index > 0 {
            data.push(' ');
        }
        match *command {
            PathCommand::MoveTo(point) => {
                data.push('M');
                push_command_point(&mut data, point);
            }
            PathCommand::LineTo(point) => {
                data.push('L');
                push_command_point(&mut data, point);
            }
            PathCommand::QuadraticTo { control, to } => {
                data.push('Q');
                push_command_point(&mut data, control);
                data.push(' ');
                push_command_point(&mut data, to);
            }
            PathCommand::CubicTo { control1, control2, to } => {
                data.push('C');
                push_command_point(&mut data, control1);
                data.push(' ');
                push_command_point(&mut data, control2);
                data.push(' ');
                push_command_point(&mut data, to);
            }
            PathCommand::Close => data.push('Z'),
        }
    }
    data
}

fn push_command_point(data: &mut String, point: Point) {
    data.push_str(&number(point.x));
    data.push(' ');
    data.push_str(&number(point.y));
}

fn number(value: f64) -> String {
    let rounded = (value * 1_000_000.0).round() / 1_000_000.0;
    if rounded == 0.0 { "0".to_string() } else { rounded.to_string() }
}

fn indent(depth: usize) -> String {
    "  ".repeat(depth)
}

fn escape(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&apos;"),
            _ => escaped.push(character),
        }
    }
    escaped
}
