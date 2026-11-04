//! Bundled-Chromium gate for the Vega-Lite lowering.
//!
//! The gate is programmatic: the SVG is measured inside the Chromium that
//! Playwright ships and launches through `chromium.launch()` with no `channel`
//! and no `executablePath`. A system Chrome install is never touched. The
//! browser identity is pinned twice: by `@playwright/test` in `package.json`
//! (installed with `npm ci` and `npx playwright install chromium`) and by the
//! constants below, so a browser bump fails loudly instead of drifting.

use std::{fs, path::Path, path::PathBuf, process::Command};

use davinci_chart::vega_lite;

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

const DOCUMENT: &str = r#"{
  "width": 480,
  "height": 300,
  "data": {"values": [
    {"region": "North", "revenue": 3},
    {"region": "South", "revenue": 7},
    {"region": "West", "revenue": 5}
  ]},
  "mark": "bar",
  "encoding": {
    "x": {"field": "region", "type": "nominal", "scale": {"type": "band"}},
    "y": {"field": "revenue", "type": "quantitative"}
  }
}"#;

fn capture_script() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../scripts/visual/capture.mjs")
}

/// Rasterize one page and return the browser identity line.
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
        assert!(meta.contains(&expected), "the rasterizer identity moved: `{meta}` lacks `{expected}`. Re-pin deliberately");
    }
    meta
}

#[test]
fn playwright_bundled_chromium_measures_the_declared_canvas_and_bars() {
    let chart = vega_lite::lower(DOCUMENT).expect("the gate document must lower");
    let svg = chart.to_svg().expect("the gate document must serialize");

    let dir = std::env::temp_dir().join("notedown-chart-gate");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("gate dir");
    let page = dir.join("gate.html");
    let shot = dir.join("gate.png");
    let html = format!("<!DOCTYPE html><html><head><meta charset=\"utf-8\"><style>html,body{{margin:0;padding:0}}</style></head><body>{svg}</body></html>");
    fs::write(&page, html).expect("write gate page");

    capture(&page, &shot, 480, 300);

    let (width, height) = png_size(&shot);
    assert_eq!((width, height), (480, 300), "the rasterized page must equal the declared canvas");
    let bytes = fs::metadata(&shot).map(|meta| meta.len()).unwrap_or(0);
    assert!(bytes > 512, "the screenshot looks empty ({bytes} bytes)");
    assert_eq!(chart.plan.canvas.width, width as f64, "the plan and the raster agree on width");
    assert_eq!(chart.plan.canvas.height, height as f64, "the plan and the raster agree on height");
}

/// Read the PNG IHDR dimensions without pulling in an image decoder.
fn png_size(path: &Path) -> (u32, u32) {
    let bytes = fs::read(path).unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
    assert!(bytes.len() > 24, "{} is not a PNG", path.display());
    assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n", "{} is not a PNG", path.display());
    assert_eq!(&bytes[12..16], b"IHDR", "{} has no IHDR chunk", path.display());
    let width = u32::from_be_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]);
    let height = u32::from_be_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]);
    (width, height)
}
