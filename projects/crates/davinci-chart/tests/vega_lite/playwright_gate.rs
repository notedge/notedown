//! Bundled-Chromium gate for the Vega-Lite lowering.
//!
//! The gate is programmatic: the SVG is measured inside the Chromium that
//! Playwright ships and launches through `chromium.launch()` with no `channel`
//! and no `executablePath`. A system Chrome install is never touched. The
//! browser identity is pinned twice: by `@playwright/test` in `package.json`
//! (installed with `npm ci` and `npx playwright install chromium`) and by the
//! constants below, so a browser bump fails loudly instead of drifting.

use std::{collections::BTreeSet, fs, path::Path, path::PathBuf, process::Command};

use davinci_chart::vega_lite;
use davinci_types::{Color, contrast_ratio};

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

/// Inspect the rendered page: every DOM id plus the resolved light/dark chrome colors.
fn inspect_script() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/inspect_dom.mjs")
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
        "`inspect_dom.mjs` failed; browser tests must use the bundled Playwright Chromium (run `npm ci` then `npx playwright install chromium`): {}",
        String::from_utf8_lossy(&output.stderr).trim()
    );
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let payload = stdout
        .lines()
        .find(|line| line.starts_with("META "))
        .map(|line| &line[5..])
        .unwrap_or_else(|| panic!("`inspect_dom.mjs` printed no payload line: {stdout}"));
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
fn two_charts_on_one_page_keep_ids_unique_and_both_palettes_clear_wcag_aa() {
    let chart = vega_lite::lower(DOCUMENT).expect("the gate document must lower");
    let left = chart.to_svg_with_hooks("chart-0", "chart").expect("first hooked svg");
    let right = chart.to_svg_with_hooks("chart-1", "chart").expect("second hooked svg");
    for svg in [&left, &right] {
        assert!(svg.contains("@media (prefers-color-scheme: dark)"), "every chart must carry both palettes: {svg}");
    }

    let dir = std::env::temp_dir().join("notedown-chart-visual-gate");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("visual gate dir");
    let page = dir.join("two-charts.html");
    let html = format!(
        "<!DOCTYPE html><html><head><meta charset=\"utf-8\"><style>html,body{{margin:0;padding:0;background:#ffffff}}@media (prefers-color-scheme: dark){{html,body{{background:#0d1117}}}}.charts{{display:flex}}</style></head><body><main class=\"charts\">{left}{right}</main></body></html>"
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
    assert_eq!(unique.len(), ids.len(), "ids must be unique across a page with two charts: {ids:?}");
    assert!(ids.iter().all(|id| id.starts_with("chart-0-") || id.starts_with("chart-1-")), "every id must be namespaced: {ids:?}");
    assert!(ids.iter().any(|id| id.starts_with("chart-0-")), "the first chart must emit ids: {ids:?}");
    assert!(ids.iter().any(|id| id.starts_with("chart-1-")), "the second chart must emit ids: {ids:?}");

    let light = &payload["modes"]["light"];
    let dark = &payload["modes"]["dark"];
    assert_ne!(
        light["tickFill"].as_str(),
        dark["tickFill"].as_str(),
        "the light and dark palettes must resolve different tick colors"
    );
    assert_ne!(
        light["axisStroke"].as_str(),
        dark["axisStroke"].as_str(),
        "the light and dark palettes must resolve different axis colors"
    );

    let light_tick = computed_rgb(light["tickFill"].as_str().expect("light tick fill"));
    let dark_tick = computed_rgb(dark["tickFill"].as_str().expect("dark tick fill"));
    let light_axis = computed_rgb(light["axisStroke"].as_str().expect("light axis stroke"));
    let dark_axis = computed_rgb(dark["axisStroke"].as_str().expect("dark axis stroke"));

    let light_page = Color::WHITE;
    let dark_page = Color::rgb(0x0D, 0x11, 0x17);
    for (label, foreground, background) in [("light tick text", light_tick, light_page), ("dark tick text", dark_tick, dark_page), ("light axis", light_axis, light_page), ("dark axis", dark_axis, dark_page)] {
        let ratio = contrast_ratio(foreground, background);
        assert!(ratio >= 4.5, "`{label}` must clear WCAG AA 4.5:1, got {ratio:.2}");
    }
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
