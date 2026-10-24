use davinci_chart::{
    Channel, ChannelEncoding, ChartOptions, ChartSpec, DataSnapshot, FieldKind, FieldRef, Mark, MarkGeometry, Scale, compile, lower, render,
};
use davinci_scene::{SceneResolver, SemanticKind, SemanticNode, SemanticRole, SemanticScene};

use crate::support::{bar_data, bar_spec, column, numbers, snapshot, texts};

fn options() -> ChartOptions {
    ChartOptions::default()
}

fn point_spec() -> ChartSpec {
    ChartSpec::new(Mark::Point)
        .with_field(FieldRef::new("x", FieldKind::Quantitative))
        .with_field(FieldRef::new("y", FieldKind::Quantitative))
        .with_field(FieldRef::new("group", FieldKind::Nominal))
        .with_encoding(Channel::X, ChannelEncoding::field("x", FieldKind::Quantitative))
        .with_encoding(Channel::Y, ChannelEncoding::field("y", FieldKind::Quantitative))
        .with_encoding(Channel::Color, ChannelEncoding::field("group", FieldKind::Nominal))
}

fn point_data() -> DataSnapshot {
    snapshot(vec![
        column("x", FieldKind::Quantitative, numbers(&[1.0, 2.0, 3.0])),
        column("y", FieldKind::Quantitative, numbers(&[4.0, 5.0, 6.0])),
        column("group", FieldKind::Nominal, texts(&["a", "b", "c"])),
    ])
}

fn find<'a>(scene: &'a SemanticScene, id: &str) -> &'a SemanticNode {
    scene.nodes.iter().find(|node| node.id.as_deref() == Some(id)).unwrap_or_else(|| panic!("missing scene node `{id}`"))
}

fn primitive(node: &SemanticNode) -> &'static str {
    match &node.kind {
        SemanticKind::Group { .. } => "group",
        SemanticKind::Path { .. } => "path",
        SemanticKind::Line { .. } => "line",
        SemanticKind::Rect { .. } => "rect",
        SemanticKind::RoundedRect { .. } => "rounded-rect",
        SemanticKind::Ellipse { .. } => "ellipse",
        SemanticKind::Text { .. } => "text",
        SemanticKind::Image { .. } => "image",
        SemanticKind::Marker { .. } => "marker",
    }
}

#[test]
fn color_channel_is_resolved_into_the_scene_marker_fill() {
    let plan = compile(&point_spec(), &point_data(), &options()).expect("point chart should compile");
    let nominal = plan.scale(Channel::Color).and_then(Scale::nominal).expect("color nominal scale");
    let scene = render(&plan);

    let node = find(&scene, "chart-point-0");
    assert_eq!(node.role, Some(SemanticRole::Mark));

    let SemanticKind::Marker { at, style, .. } = &node.kind
    else {
        panic!("a point mark must render as a marker");
    };
    let expected = nominal.color("a").expect("category a");
    assert_eq!(style.fill, Some(expected), "the resolved fill is the nominal color");

    let MarkGeometry::Point { center, .. } = &plan.marks[0].geometry
    else {
        panic!("a point must compile to a marker geometry");
    };
    assert_eq!(*at, *center, "the scene marker keeps the plan center");

    assert_eq!(
        node.source.as_ref().and_then(|source| source.element.as_deref()),
        Some("1"),
        "the source descriptor is the row, not the series name"
    );
}

#[test]
fn the_semantic_scene_holds_only_display_primitives() {
    let plan = compile(&bar_spec(), &bar_data(), &options()).expect("bar chart should compile");
    let scene = render(&plan);

    assert!(plan.scale(Channel::X).is_some(), "a plan may carry trained scales");

    let mut kinds: Vec<&'static str> = Vec::new();
    for node in &scene.nodes {
        let kind = primitive(node);
        if !kinds.contains(&kind) {
            kinds.push(kind);
        }
    }
    kinds.sort_unstable();
    assert_eq!(kinds, vec!["line", "rect", "text"], "only display primitives reach the scene");

    for index in 0..3 {
        let node = find(&scene, &format!("chart-bar-{index}"));
        let SemanticKind::Rect { rect, .. } = &node.kind
        else {
            panic!("a bar must render as a rectangle");
        };
        let MarkGeometry::Rect(expected) = &plan.marks[index].geometry
        else {
            panic!("a bar must compile to a rectangle");
        };
        assert_eq!(rect, expected, "the scene keeps final geometry only");
    }
}

#[test]
fn svg_serialization_requires_the_display_scene() {
    let lowered = lower(&bar_spec(), &bar_data(), &options()).expect("bar chart should lower");
    assert_eq!(lowered.diagnostics, lowered.plan.diagnostics, "diagnostics come from the plan");

    let display = lowered.scene.resolve(&SceneResolver::default());
    display.validate().expect("the display scene must be valid");
    let svg = display.to_svg().expect("the display scene must serialize");

    assert!(svg.starts_with("<svg"), "the display scene owns SVG output");
    assert!(svg.contains("data-role=\"mark\""), "marks keep their semantic role in SVG");
    assert!(svg.contains("data-role=\"axis\""), "axes keep their semantic role in SVG");
    assert!(svg.contains(">A</text>"), "category labels reach the SVG text");
    assert_eq!(svg, display.to_svg().expect("repeated serialization must agree"));
}

#[test]
fn axes_and_ticks_keep_their_semantic_roles() {
    let plan = compile(&bar_spec(), &bar_data(), &options()).expect("bar chart should compile");
    let scene = render(&plan);

    let axis = find(&scene, "chart-axis-x");
    assert_eq!(axis.role, Some(SemanticRole::Axis));
    assert!(matches!(axis.kind, SemanticKind::Line { .. }));

    let tick = find(&scene, "chart-x-tick-0");
    assert_eq!(tick.role, Some(SemanticRole::Tick));
    assert!(matches!(&tick.kind, SemanticKind::Text { text, .. } if text == "A"), "band ticks are labeled by category");

    let bar = find(&scene, "chart-bar-0");
    assert_eq!(bar.role, Some(SemanticRole::Mark));
    assert!(bar.hit_region.is_some(), "mark hit regions are display attributes");
}

#[test]
fn rendering_the_same_plan_is_deterministic() {
    let plan = compile(&point_spec(), &point_data(), &options()).expect("point chart should compile");
    assert_eq!(render(&plan), render(&plan), "render must be a pure function");
}
