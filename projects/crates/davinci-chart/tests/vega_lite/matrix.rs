//! Mark × channel golden matrix for the Vega-Lite subset.
//!
//! Every cell either renders a real SVG with the declared canvas size or is
//! refused by the kernel with a structured code. Artifacts land in
//! `CARGO_TARGET_TMPDIR/vega-artifacts` so a reviewer can open the exact SVG
//! the matrix produced.

use std::{fs, path::PathBuf};

use davinci_chart::{codes, vega_lite};

const MARKS: &[&str] = &["bar", "line", "area", "point", "rect", "tick"];

const CHANNELS: &[&str] = &["x", "y", "color", "size", "opacity"];

const ROWS: &str = r#"[
  {"region": "A", "segment": "upper", "step": 1, "value": 3, "group": "g1", "alpha": 0.35},
  {"region": "B", "segment": "lower", "step": 2, "value": 7, "group": "g2", "alpha": 0.55},
  {"region": "C", "segment": "upper", "step": 3, "value": 5, "group": "g1", "alpha": 0.75},
  {"region": "D", "segment": "lower", "step": 4, "value": 9, "group": "g2", "alpha": 0.95}
]"#;

/// The base encoding pair that keeps each mark inside its kernel contract.
fn base(mark: &str) -> (&'static str, &'static str) {
    match mark {
        "bar" | "point" | "tick" => (
            r#""x": {"field": "region", "type": "nominal", "scale": {"type": "band"}}"#,
            r#""y": {"field": "value", "type": "quantitative"}"#,
        ),
        "rect" => (
            r#""x": {"field": "region", "type": "nominal"}"#,
            r#""y": {"field": "segment", "type": "nominal"}"#,
        ),
        _ => (r#""x": {"field": "step", "type": "quantitative"}"#, r#""y": {"field": "value", "type": "quantitative"}"#),
    }
}

fn channel_fragment(channel: &str) -> &'static str {
    match channel {
        "color" => r#""color": {"field": "group", "type": "nominal"}"#,
        "size" => r#""size": {"field": "value", "type": "quantitative"}"#,
        "opacity" => r#""opacity": {"field": "alpha", "type": "quantitative"}"#,
        _ => "",
    }
}

fn document(mark: &str, channel: &str) -> String {
    let (x, y) = base(mark);
    let extra = channel_fragment(channel);
    let encoding = if extra.is_empty() { format!("{x}, {y}") } else { format!("{x}, {y}, {extra}") };
    format!(r#"{{"width": 640, "height": 400, "data": {{"values": {ROWS}}}, "mark": "{mark}", "encoding": {{{encoding}}}}}"#)
}

fn artifact_dir() -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("vega-artifacts");
    fs::create_dir_all(&dir).expect("create vega artifact dir");
    dir
}

const BASE_DOCUMENT: &str = r#"{"data":{"values":[{"c":"A","v":3},{"c":"B","v":7}]},"mark":"bar","encoding":{"x":{"field":"c","type":"nominal"},"y":{"field":"v","type":"quantitative"}}}"#;

#[test]
fn constant_mark_styling_changes_paint_without_touching_geometry() {
    let plain = vega_lite::lower(BASE_DOCUMENT).expect("the plain bar must lower");
    let styled = vega_lite::lower(
        r##"{"data":{"values":[{"c":"A","v":3},{"c":"B","v":7}]},"mark":{"type":"bar","color":"#3366cc","opacity":0.5},"encoding":{"x":{"field":"c","type":"nominal"},"y":{"field":"v","type":"quantitative"}}}"##,
    )
    .expect("constant mark styling must lower");

    assert_eq!(styled.plan.marks.len(), plain.plan.marks.len(), "styling adds no mark instances");
    for (before, after) in plain.plan.marks.iter().zip(&styled.plan.marks) {
        assert_eq!(before.geometry, after.geometry, "styling must not move geometry");
        assert_eq!(after.style.fill, Some(davinci_scene::Color::rgb(0x33, 0x66, 0xcc)));
        assert_eq!(after.style.opacity, 0.5);
    }
}

/// The kernel binds every channel on every mark except `size`, which only
/// `point` consumes.
fn kernel_supports(mark: &str, channel: &str) -> bool {
    channel != "size" || mark == "point"
}

#[test]
fn every_mark_channel_cell_renders_or_is_refused_with_the_kernel_contract() {
    let dir = artifact_dir();
    let mut rendered = Vec::new();
    let mut refused = Vec::new();

    for mark in MARKS {
        for channel in CHANNELS {
            let source = document(mark, channel);
            match vega_lite::lower(&source) {
                Ok(chart) => {
                    assert_eq!(chart.plan.canvas.width, 640.0, "{mark}/{channel}: declared width must reach the plan");
                    assert_eq!(chart.plan.canvas.height, 400.0, "{mark}/{channel}: declared height must reach the plan");
                    assert!(!chart.plan.marks.is_empty(), "{mark}/{channel}: a rendered cell needs mark instances");

                    let svg = chart.to_svg().unwrap_or_else(|error| panic!("{mark}/{channel}: scene must serialize: {error}"));
                    assert!(svg.contains("<svg"), "{mark}/{channel}: missing svg root");
                    assert!(svg.len() > 200, "{mark}/{channel}: svg looks empty ({} bytes)", svg.len());
                    let artifact = dir.join(format!("vega-{mark}-{channel}.svg"));
                    fs::write(&artifact, &svg).expect("write vega svg artifact");
                    eprintln!("wrote {} ({} bytes)", artifact.display(), svg.len());
                    rendered.push((*mark, *channel));
                }
                Err(error) => {
                    assert_eq!(error.code(), codes::INVALID_SPEC, "{mark}/{channel}: expected a kernel refusal, got `{}`", error.message());
                    refused.push((*mark, *channel));
                }
            }
        }
    }

    for (mark, channel) in &rendered {
        assert!(kernel_supports(mark, channel), "{mark}/{channel}: the kernel does not support this binding yet the cell rendered");
    }
    for (mark, channel) in &refused {
        assert!(!kernel_supports(mark, channel), "{mark}/{channel}: the kernel supports this binding yet the cell was refused");
    }
    assert_eq!(rendered.len() + refused.len(), MARKS.len() * CHANNELS.len());
    assert_eq!(rendered.len(), 25, "five cells bind `size` on a mark that does not consume it");
    eprintln!("matrix: {} rendered, {} refused", rendered.len(), refused.len());
}

#[test]
fn every_base_document_renders_byte_identical_svgs() {
    for mark in MARKS {
        let source = document(mark, "x");
        let first = vega_lite::lower(&source).unwrap_or_else(|error| panic!("{mark}: first lowering failed: {error}"));
        let second = vega_lite::lower(&source).unwrap_or_else(|error| panic!("{mark}: second lowering failed: {error}"));
        assert_eq!(
            first.to_svg().expect("first svg"),
            second.to_svg().expect("second svg"),
            "{mark}: the same document must render byte-identical output"
        );
    }
}

#[test]
fn refusing_documents_never_emit_an_svg() {
    let error = vega_lite::lower(&BASE_DOCUMENT.replace(r#""mark":"bar""#, r#""mark":"rule""#)).expect_err("rule is outside the subset");
    assert_eq!(error.code(), codes::UNSUPPORTED_MARK);
}
