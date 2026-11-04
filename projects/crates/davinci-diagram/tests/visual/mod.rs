//! Fixed-rasterizer visual gate for diagram scenes.
//!
//! The gate has two halves and both run on the same fixtures:
//!
//! 1. Geometry is asserted straight from the plan: no node, cluster, label box
//!    or routed point may leave the canvas, and no route may enter a
//!    non-endpoint node (inflated by the label clearance), which is what
//!    "nothing is clipped and nothing is hidden" means in numbers.
//! 2. Pixels are captured by the bundled Playwright Chromium and compared
//!    against the recorded digest, so a rendering change cannot pass unnoticed.
//!
//! `scripts/visual/capture.mjs` calls `chromium.launch()` with no `channel`
//! and no `executablePath`: a system Chrome install is never touched. The
//! browser comes from `npm ci` plus `npx playwright install chromium`.
//!
//! Re-record deliberately with `NOTEDOWN_VISUAL_RECORD=1 cargo test -p
//! davinci-diagram --test main visual`. Bumping Playwright or Chromium is a
//! deliberate act: the identity below is the identity of the recorded pixels.

use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use davinci_diagram::{DiagramError, LoweredDiagram, mermaid_to_scene};
use davinci_scene::{Point, Rect};
use davinci_types::{Color, contrast_ratio};

/// One fixture and the frontend that lowers it.
type Lower = fn(&str) -> Result<LoweredDiagram, DiagramError>;

/// The `@playwright/test` version that owns the bundled browser.
const PLAYWRIGHT_VERSION: &str = "1.64.1";
/// The Playwright browser registry revision for the bundled Chromium.
const CHROMIUM_REVISION: &str = "1248";
/// The Chrome-for-Testing build inside that revision.
const CHROMIUM_BUILD: &str = "156.0.8078.4";
/// Screenshot scale factor.
const DEVICE_SCALE_FACTOR: &str = "1";
/// Emulated `prefers-color-scheme`.
const COLOR_SCHEME: &str = "light";
/// Inflate factor used when checking that a route clears a node's stroke.
const NODE_CLEARANCE: f64 = 2.0;
/// Padding around the canvas inside the screenshot window.
const PADDING: f64 = 8.0;
/// Smallest window the rasterizer actually paints into.
const MIN_WINDOW: (u64, u64) = (400, 300);

fn fixtures() -> Vec<(&'static str, &'static str, Lower)> {
    #[allow(unused_mut)]
    let mut fixtures: Vec<(&'static str, &'static str, Lower)> = vec![
        (
            "flowchart-branch",
            "flowchart TD\n  A[Start] --> B{Ready?}\n  B -->|yes| C[Stop]\n  B -->|no| D[Retry]\n  D --> B\n",
            mermaid_to_scene,
        ),
        ("flowchart-bypass", "flowchart LR\n  A --> B\n  A --> C\n  A --> D\n  C --> E\n", mermaid_to_scene),
        ("class-members", "classDiagram\n  class A {\n    +run()\n    -state\n  }\n  A <|-- B\n  B *-- C\n", mermaid_to_scene),
        ("sequence-flow", "sequenceDiagram\n  participant A\n  participant B\n  A->>B: hello\n  B-->>A: world\n", mermaid_to_scene),
        ("subgraph-group", "flowchart LR\n  subgraph group[Group]\n    A --> B\n  end\n", mermaid_to_scene),
    ];
    #[cfg(feature = "dot")]
    fixtures.extend([
        (
            "dot-digraph",
            "digraph rank {\n  rankdir=LR;\n  node [shape=box];\n  Start [shape=circle];\n  Check [label=\"Ready?\"];\n  Work [label=\"Retry\"];\n  Start -> Check [label=\"go\"];\n  Check -> Work [label=\"no\"];\n  Work -> Check;\n}\n",
            davinci_diagram::dot_to_scene as Lower,
        ),
        (
            "dot-cluster",
            "digraph groups {\n  subgraph cluster_core {\n    label=\"Core\";\n    A -> B;\n  }\n  B -> C;\n}\n",
            davinci_diagram::dot_to_scene,
        ),
    ]);
    fixtures
}

fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("fixtures").join("visual")
}

fn manifest_path() -> PathBuf {
    // No `.txt` suffix: the repository ignores every `*.txt` file, and a
    // silently ignored manifest would turn the pixel gate off.
    fixture_dir().join("manifest")
}

fn capture_script() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../scripts/visual/capture.mjs")
}

/// Rasterize one page with the bundled Chromium and return the identity line.
fn capture(html: &Path, png: &Path, width: u64, height: u64) -> String {
    let output = Command::new("node")
        .arg(capture_script())
        .arg("--html")
        .arg(html)
        .arg("--png")
        .arg(png)
        .arg("--width")
        .arg(width.to_string())
        .arg("--height")
        .arg(height.to_string())
        .arg("--dpr")
        .arg(DEVICE_SCALE_FACTOR)
        .arg("--color-scheme")
        .arg(COLOR_SCHEME)
        .output()
        .unwrap_or_else(|error| {
            panic!("browser tests must use the bundled Playwright Chromium: run `npm ci` then `npx playwright install chromium` before this gate: {error}")
        });
    assert!(
        output.status.success(),
        "`capture.mjs` failed; browser tests must use the bundled Playwright Chromium (run `npm ci` then `npx playwright install chromium`): {}",
        String::from_utf8_lossy(&output.stderr).trim()
    );
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let meta = stdout
        .lines()
        .find(|line| line.starts_with("META "))
        .unwrap_or_else(|| panic!("`capture.mjs` printed no identity line: {stdout}"))
        .to_string();
    for expected in [
        format!("playwright={PLAYWRIGHT_VERSION}"),
        format!("chromium={CHROMIUM_BUILD}"),
        format!("revision={CHROMIUM_REVISION}"),
        format!("dpr={DEVICE_SCALE_FACTOR}"),
        format!("color-scheme={COLOR_SCHEME}"),
    ] {
        assert!(meta.contains(&expected), "the rasterizer identity moved: `{meta}` lacks `{expected}`. Re-record deliberately");
    }
    meta
}

/// Stable 128-bit digest without pulling a hashing crate into the workspace.
fn digest(bytes: &[u8]) -> String {
    let mut lanes = [0xcbf29ce484222325u64, 0x84222325cbf29ce4u64];
    for (lane, state) in lanes.iter_mut().enumerate() {
        for byte in bytes {
            *state ^= u64::from(*byte).wrapping_mul(1 + lane as u64);
            *state = state.wrapping_mul(0x100000001b3);
        }
        *state ^= bytes.len() as u64;
    }
    lanes.iter().map(|lane| format!("{lane:016x}")).collect()
}

fn inside(canvas: (f64, f64), rect: Rect) -> bool {
    rect.x >= 0.0 && rect.y >= 0.0 && rect.right() <= canvas.0 && rect.bottom() <= canvas.1
}

fn inflated(rect: Rect, by: f64) -> Rect {
    Rect::new(rect.x - by, rect.y - by, rect.width + 2.0 * by, rect.height + 2.0 * by)
}

fn segment_enters(a: Point, b: Point, rect: Rect) -> bool {
    let (x0, x1) = (a.x.min(b.x), a.x.max(b.x));
    let (y0, y1) = (a.y.min(b.y), a.y.max(b.y));
    x0 <= rect.right() && x1 >= rect.x && y0 <= rect.bottom() && y1 >= rect.y
}

#[test]
fn no_construct_leaves_the_canvas_and_no_route_hides_behind_a_node() {
    for (id, source, lower) in fixtures() {
        let lowered = lower(source).unwrap_or_else(|error| panic!("{id} should lower: {error}"));
        let canvas = (lowered.plan.canvas.width, lowered.plan.canvas.height);

        for node in &lowered.plan.nodes {
            assert!(inside(canvas, node.rect), "{id}: node `{}` leaves the canvas: {:?}", node.id, node.rect);
        }
        for cluster in &lowered.plan.clusters {
            assert!(inside(canvas, cluster.rect), "{id}: cluster `{}` leaves the canvas: {:?}", cluster.id, cluster.rect);
        }
        for edge in &lowered.plan.edges {
            for point in &edge.points {
                assert!(
                    point.x >= 0.0 && point.y >= 0.0 && point.x <= canvas.0 && point.y <= canvas.1,
                    "{id}: edge `{}->{}` leaves the canvas at {point:?}",
                    edge.from.node,
                    edge.to.node
                );
            }
            if let Some(label) = edge.label_box {
                assert!(inside(canvas, label), "{id}: label plate of `{}->{}` leaves the canvas: {label:?}", edge.from.node, edge.to.node);
            }

            let endpoints = [edge.from.node.as_str(), edge.to.node.as_str()];
            for pair in edge.points.windows(2) {
                for node in &lowered.plan.nodes {
                    if endpoints.contains(&node.id.as_str()) {
                        continue;
                    }
                    assert!(
                        !segment_enters(pair[0], pair[1], inflated(node.rect, NODE_CLEARANCE)),
                        "{id}: edge `{}->{}` passes through node `{}`: segment {:?}->{:?} vs {:?}",
                        edge.from.node,
                        edge.to.node,
                        node.id,
                        pair[0],
                        pair[1],
                        node.rect
                    );
                }
            }
        }
    }
}

/// Inspect the rendered page: every DOM id plus the resolved light/dark palette colors.
fn inspect_script() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/inspect_diagram.mjs")
}

fn inspect(html: &Path) -> serde_json::Value {
    let output = Command::new("node")
        .arg(inspect_script())
        .arg("--html")
        .arg(html)
        .output()
        .unwrap_or_else(|error| {
            panic!("browser tests must use the bundled Playwright Chromium: run `npm ci` then `npx playwright install chromium` before this gate: {error}")
        });
    assert!(
        output.status.success(),
        "`inspect_diagram.mjs` failed; browser tests must use the bundled Playwright Chromium (run `npm ci` then `npx playwright install chromium`): {}",
        String::from_utf8_lossy(&output.stderr).trim()
    );
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let payload = stdout
        .lines()
        .find(|line| line.starts_with("META "))
        .map(|line| &line[5..])
        .unwrap_or_else(|| panic!("`inspect_diagram.mjs` printed no payload line: {stdout}"));
    serde_json::from_str(payload).expect("the inspect payload is JSON")
}

/// Parse a `getComputedStyle` color such as `rgb(31, 35, 40)`.
fn computed_rgb(value: &str) -> Color {
    let inner = value
        .strip_prefix("rgb(")
        .and_then(|rest| rest.strip_suffix(')'))
        .unwrap_or_else(|| panic!("the computed color `{value}` must be `rgb(...)`"));
    let channels: Vec<u8> = inner
        .split(',')
        .map(|channel| channel.trim().parse().unwrap_or_else(|_| panic!("the computed color `{value}` has an 8-bit channel")))
        .collect();
    assert_eq!(channels.len(), 3, "the computed color `{value}` must have three channels");
    Color::rgb(channels[0], channels[1], channels[2])
}

#[test]
fn two_diagrams_on_one_page_keep_ids_unique_and_both_palettes_clear_wcag_aa() {
    let (_, source, lower) = fixtures().into_iter().find(|(id, _, _)| *id == "flowchart-branch").expect("the gate fixture exists");
    let lowered = lower(source).expect("the gate fixture must lower");
    let theme = &davinci_types::DIAGRAM_THEME;
    let left = lowered.scene.to_svg_with_hooks("diagram-0", "d", theme).expect("first hooked svg");
    let right = lowered.scene.to_svg_with_hooks("diagram-1", "d", theme).expect("second hooked svg");
    for svg in [&left, &right] {
        assert!(svg.contains("@media (prefers-color-scheme: dark)"), "every diagram must carry both palettes: {svg}");
    }

    let dir = std::env::temp_dir().join("notedown-diagram-visual-gate");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("visual gate dir");
    let page = dir.join("two-diagrams.html");
    let html = format!(
        "<!DOCTYPE html><html><head><meta charset=\"utf-8\"><style>html,body{{margin:0;padding:0;background:#ffffff}}@media (prefers-color-scheme: dark){{html,body{{background:#0d1117}}}}.diagrams{{display:flex}}</style></head><body><main class=\"diagrams\">{left}{right}</main></body></html>"
    );
    fs::write(&page, html).expect("write visual gate page");

    let payload = inspect(&page);
    let ids: Vec<&str> = payload["ids"]
        .as_array()
        .expect("the payload carries ids")
        .iter()
        .map(|id| id.as_str().expect("every id is a string"))
        .collect();
    let unique: BTreeSet<&str> = ids.iter().copied().collect();
    assert_eq!(unique.len(), ids.len(), "ids must be unique across a page with two diagrams: {ids:?}");
    assert!(ids.iter().all(|id| id.starts_with("diagram-0-") || id.starts_with("diagram-1-")), "every id must be namespaced: {ids:?}");
    assert!(ids.iter().any(|id| id.starts_with("diagram-0-")), "the first diagram must emit ids: {ids:?}");
    assert!(ids.iter().any(|id| id.starts_with("diagram-1-")), "the second diagram must emit ids: {ids:?}");

    let light = &payload["modes"]["light"];
    let dark = &payload["modes"]["dark"];
    for key in ["nodeFill", "nodeLabelFill", "edgeStroke", "edgeLabelFill"] {
        assert_ne!(
            light[key].as_str(),
            dark[key].as_str(),
            "the light and dark palettes must resolve different `{key}` colors"
        );
        assert!(light[key].is_string(), "the light palette must resolve `{key}`");
        assert!(dark[key].is_string(), "the dark palette must resolve `{key}`");
    }

    let light_page = Color::WHITE;
    let dark_page = Color::rgb(0x0D, 0x11, 0x17);
    let pairs = [
        ("light node label", "nodeLabelFill", "nodeFill", light, light_page),
        ("dark node label", "nodeLabelFill", "nodeFill", dark, dark_page),
        ("light edge label", "edgeLabelFill", "page", light, light_page),
        ("dark edge label", "edgeLabelFill", "page", dark, dark_page),
        ("light edge line", "edgeStroke", "page", light, light_page),
        ("dark edge line", "edgeStroke", "page", dark, dark_page),
    ];
    for (label, foreground_key, background_key, mode, page_color) in pairs {
        let foreground = computed_rgb(mode[foreground_key].as_str().expect("foreground color"));
        let background = match background_key {
            "page" => page_color,
            other => computed_rgb(mode[other].as_str().expect("background color")),
        };
        let ratio = contrast_ratio(foreground, background);
        assert!(ratio >= 4.5, "`{label}` must clear WCAG AA 4.5:1, got {ratio:.2}");
    }
}

#[test]
fn bundled_chromium_pixels_match_the_recorded_manifest() {
    let record = std::env::var("NOTEDOWN_VISUAL_RECORD").is_ok();
    let manifest = if record { String::new() } else { fs::read_to_string(manifest_path()).expect("record the visual manifest first") };
    let recorded: BTreeSet<String> =
        manifest.lines().filter(|line| !line.trim().is_empty() && !line.starts_with('#')).map(str::to_string).collect();

    let work = std::env::temp_dir().join("notedown-visual-gate");
    let _ = fs::remove_dir_all(&work);
    fs::create_dir_all(&work).expect("temp workspace");

    let mut current = Vec::new();
    for (id, source, lower) in fixtures() {
        let lowered = lower(source).unwrap_or_else(|error| panic!("{id} should lower: {error}"));
        let svg = lowered.scene.to_svg().expect("scene should serialize");
        let width = lowered.plan.canvas.width + 2.0 * PADDING;
        let height = lowered.plan.canvas.height + 2.0 * PADDING;

        let html = work.join(format!("{id}.html"));
        fs::write(
            &html,
            format!(
                "<!doctype html><html><head><meta charset=\"utf-8\"><style>html,body{{margin:0;padding:0;background:#fff}}#f{{padding:{PADDING}px;line-height:0;display:inline-block}}</style></head><body><div id=\"f\">{svg}</div></body></html>"
            ),
        )
        .expect("wrapper html");

        let png = work.join(format!("{id}.png"));
        // A minimum viewport keeps every fixture on the same recorded frame
        // size the old gate used, so only the browser identity can change a
        // digest.
        let window_width = (width.ceil() as u64).max(MIN_WINDOW.0);
        let window_height = (height.ceil() as u64).max(MIN_WINDOW.1);
        capture(&html, &png, window_width, window_height);
        let bytes = fs::read(&png).unwrap_or_else(|error| panic!("{id}: no screenshot: {error}"));
        assert!(bytes.len() > 1024, "{id}: screenshot is suspiciously small");

        let line = format!("{id} {} {window_width}x{window_height}", digest(&bytes));
        if record {
            let out = fixture_dir().join(format!("{id}.png"));
            fs::create_dir_all(fixture_dir()).expect("fixture dir");
            fs::write(&out, &bytes).expect("record png");
            current.push(line);
        }
        else {
            assert!(recorded.contains(&line), "{id}: pixels changed, expected one of {recorded:?} but got `{line}`");
        }
    }

    if record {
        fs::write(
            manifest_path(),
            format!(
                "# playwright {PLAYWRIGHT_VERSION} chromium {CHROMIUM_BUILD} revision {CHROMIUM_REVISION} dpr {DEVICE_SCALE_FACTOR} color-scheme {COLOR_SCHEME}\n{}\n",
                current.join("\n")
            ),
        )
        .expect("write manifest");
    }
}
