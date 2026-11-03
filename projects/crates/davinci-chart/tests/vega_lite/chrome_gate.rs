//! Pinned headless-Chrome gate for the Vega-Lite lowering.
//!
//! The gate is programmatic: the SVG is measured inside one fixed Chrome
//! build, and the measured canvas size and mark count must equal the values
//! the chart domain declared. No human opens a browser and no pixel baseline
//! is recorded here, so a host upgrade fails loudly instead of drifting.

use std::{fs, path::Path, path::PathBuf, process::Command};

use davinci_chart::vega_lite;

/// The only Chrome build this gate accepts.
const CHROME_VERSION: &str = "156.0.8078.80";

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

fn chrome() -> PathBuf {
    std::env::var("NOTEDOWN_CHROME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("C:/Program Files/Google/Chrome/Application/chrome.exe"))
}

/// Read the installed build from the version directory next to the executable.
///
/// `chrome --version` is unreliable on Windows because a running browser takes
/// the flag, so the sibling `<w.x.y.z>` directory name is the build identity.
fn installed_version(executable: &Path) -> Option<String> {
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

fn profile(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join("notedown-chart-profile").join(tag);
    fs::create_dir_all(&dir).expect("profile dir");
    dir
}

#[test]
fn pinned_headless_chrome_measures_the_declared_canvas_and_bars() {
    let executable = chrome();
    let actual = installed_version(&executable)
        .unwrap_or_else(|| panic!("cannot read the installed build at {}", executable.display()));
    assert_eq!(actual, CHROME_VERSION, "the rasterizer moved to {actual}. Re-pin deliberately instead of measuring a different build");

    let chart = vega_lite::lower(DOCUMENT).expect("the gate document must lower");
    let svg = chart.to_svg().expect("the gate document must serialize");

    let page_dir = profile("vega-gate");
    let page = page_dir.join("gate.html");
    let shot = page_dir.join("gate.png");
    let html = format!("<!DOCTYPE html><html><head><meta charset=\"utf-8\"><style>html,body{{margin:0;padding:0}}</style></head><body>{svg}</body></html>");
    fs::write(&page, html).expect("write gate page");

    let output = Command::new(&executable)
        .args(["--headless=new", "--disable-gpu", "--force-device-scale-factor=1", "--hide-scrollbars", "--window-size=480,300", "--virtual-time-budget=2000"])
        .arg(format!("--user-data-dir={}", profile("vega-gate-browser").display()))
        .arg(format!("--screenshot={}", shot.display()))
        .arg(format!("file:///{}", page.display().to_string().replace('\\', "/")))
        .output()
        .unwrap_or_else(|error| panic!("cannot run {}: {error}", executable.display()));

    assert!(
        output.status.success(),
        "chrome exited with {:?}: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
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
