//! Fixed-rasterizer visual gate for diagram scenes.
//!
//! The gate has two halves and both run on the same fixtures:
//!
//! 1. Geometry is asserted straight from the plan: no node, cluster, label box
//!    or routed point may leave the canvas, and no route may enter a
//!    non-endpoint node (inflated by the label clearance), which is what
//!    "nothing is clipped and nothing is hidden" means in numbers.
//! 2. Pixels are captured by the pinned headless Chrome and compared against
//!    the recorded digest, so a rendering change cannot pass unnoticed.
//!
//! Re-record deliberately with `NOTEDOWN_VISUAL_RECORD=1 cargo test -p
//! davinci-diagram --test main visual`. Bumping Chrome is a deliberate act: the
//! version below is the identity of the recorded pixels.

use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use davinci_diagram::mermaid_to_scene;
use davinci_scene::{Point, Rect};

/// Rasterizer identity. A different host build must re-record on purpose.
const CHROME_VERSION: &str = "156.0.8078.80";
/// Inflate factor used when checking that a route clears a node's stroke.
const NODE_CLEARANCE: f64 = 2.0;
/// Padding around the canvas inside the screenshot window.
const PADDING: f64 = 8.0;
/// Smallest window the rasterizer actually paints into.
const MIN_WINDOW: (u64, u64) = (400, 300);

fn fixtures() -> Vec<(&'static str, &'static str)> {
    vec![
        ("flowchart-branch", "flowchart TD\n  A[Start] --> B{Ready?}\n  B -->|yes| C[Stop]\n  B -->|no| D[Retry]\n  D --> B\n"),
        ("flowchart-bypass", "flowchart LR\n  A --> B\n  A --> C\n  A --> D\n  C --> E\n"),
        ("class-members", "classDiagram\n  class A {\n    +run()\n    -state\n  }\n  A <|-- B\n  B *-- C\n"),
        ("sequence-flow", "sequenceDiagram\n  participant A\n  participant B\n  A->>B: hello\n  B-->>A: world\n"),
        ("subgraph-group", "flowchart LR\n  subgraph group[Group]\n    A --> B\n  end\n"),
    ]
}

fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("fixtures").join("visual")
}

fn manifest_path() -> PathBuf {
    // No `.txt` suffix: the repository ignores every `*.txt` file, and a
    // silently ignored manifest would turn the pixel gate off.
    fixture_dir().join("manifest")
}

fn chrome() -> PathBuf {
    std::env::var("NOTEDOWN_CHROME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("C:/Program Files/Google/Chrome/Application/chrome.exe"))
}

/// An isolated profile per capture.
///
/// Chrome forwards a second invocation to a live instance that shares the same
/// profile, so a screenshot can land on a blank page while the previous capture
/// is still shutting down. One directory per fixture keeps every capture on its
/// own browser.
fn profile(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join("notedown-visual-profile").join(tag);
    fs::create_dir_all(&dir).expect("profile dir");
    dir
}

/// Read the installed build from the version directory next to the executable.
///
/// `chrome --version` is unreliable on Windows: when a browser instance is
/// already running the flag is forwarded to it and prints nothing, which is
/// exactly how a pinned gate turns into a silent host dependency. Chrome keeps
/// each build in `<Application>/<w.x.y.z>/`, so the sibling directory name is
/// the build identity. `NOTEDOWN_CHROME_VERSION` overrides it for a custom
/// launcher.
fn chrome_version(executable: &Path) -> Option<String> {
    if let Ok(version) = std::env::var("NOTEDOWN_CHROME_VERSION") {
        return Some(version);
    }
    let root = executable.parent()?;
    let mut installed: Vec<(Vec<u32>, String)> = fs::read_dir(root)
        .ok()?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().to_string();
            let parts: Vec<&str> = name.split('.').collect();
            if parts.len() != 4 || !parts.iter().all(|part| part.chars().all(|c| c.is_ascii_digit())) {
                return None;
            }
            let numbers = parts.iter().map(|part| part.parse::<u32>().ok()).collect::<Option<Vec<u32>>>()?;
            Some((numbers, name))
        })
        .collect();
    installed.sort();
    installed.pop().map(|(_, version)| version)
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
    for (id, source) in fixtures() {
        let lowered = mermaid_to_scene(source).unwrap_or_else(|error| panic!("{id} should lower: {error}"));
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

#[test]
fn pinned_chrome_pixels_match_the_recorded_manifest() {
    let executable = chrome();
    let actual = chrome_version(&executable).unwrap_or_else(|| panic!("cannot read the installed build at {}", executable.display()));
    assert_eq!(actual, CHROME_VERSION, "the rasterizer moved to {actual}. Re-record deliberately instead of mixing pixels from two builds");

    let record = std::env::var("NOTEDOWN_VISUAL_RECORD").is_ok();
    let manifest = if record { String::new() } else { fs::read_to_string(manifest_path()).expect("record the visual manifest first") };
    let recorded: BTreeSet<String> =
        manifest.lines().filter(|line| !line.trim().is_empty() && !line.starts_with('#')).map(str::to_string).collect();

    let work = std::env::temp_dir().join("notedown-visual-gate");
    let _ = fs::remove_dir_all(&work);
    fs::create_dir_all(&work).expect("temp workspace");

    let mut current = Vec::new();
    for (id, source) in fixtures() {
        let lowered = mermaid_to_scene(source).unwrap_or_else(|error| panic!("{id} should lower: {error}"));
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
        let url = format!("file:///{}", html.to_string_lossy().replace('\\', "/"));
        // Headless Chrome paints nothing in a window below roughly 400x300,
        // so a small diagram still gets a minimum window and settles in the
        // top-left corner of a white frame.
        let window_width = (width.ceil() as u64).max(MIN_WINDOW.0);
        let window_height = (height.ceil() as u64).max(MIN_WINDOW.1);
        let status = Command::new(&executable)
            .args([
                "--headless",
                "--disable-gpu",
                "--no-sandbox",
                "--hide-scrollbars",
                "--no-first-run",
                "--force-device-scale-factor=1",
                &format!("--user-data-dir={}", profile(id).to_string_lossy()),
                &format!("--window-size={window_width},{window_height}"),
                "--virtual-time-budget=2000",
                &format!("--screenshot={}", png.to_string_lossy()),
                &url,
            ])
            .status()
            .expect("rasterizer should run");
        assert!(status.success(), "{id}: rasterizer exited with {status}");
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
        fs::write(manifest_path(), format!("# pinned headless-chrome {CHROME_VERSION}\n{}\n", current.join("\n"))).expect("write manifest");
    }
}
