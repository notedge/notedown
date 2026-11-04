//! Repository gate: browser-driven tests must use the bundled Playwright
//! Chromium.
//!
//! `chromium.launch()` without `channel` and without `executablePath` resolves
//! the browser installed by `npx playwright install chromium`, pinned by
//! `@playwright/test` in `package.json`. Reaching for a system Chrome install,
//! a shell launcher, or a raw browser CLI is refused here with the rule spelled
//! out, so the failure message itself is the instruction.

use std::{
    fs,
    path::{Path, PathBuf},
};

const MESSAGE: &str = "browser tests must use the bundled Playwright Chromium: `chromium.launch()` with no `channel` and no `executablePath`, installed by `npm ci` then `npx playwright install chromium`. A system Chrome install, a shell launcher, or a raw browser CLI is forbidden.";

/// Patterns that only appear when a test or script escapes the bundled browser.
///
/// The patterns stay narrow on purpose: `channel:` alone is a domain field in
/// `davinci-chart` / `davinci-motion`, and a prose mention of `executablePath`
/// in a doc comment is not a launch. Only an actual assignment counts.
const FORBIDDEN: &[(&str, &str)] = &[
    ("chrome.exe", "system Chrome binary"),
    ("google-chrome", "system Chrome binary"),
    ("program files/google", "hard-coded browser install path"),
    ("program files\\google", "hard-coded browser install path"),
    ("notedown_chrome", "bare-metal Chrome hook"),
    ("start chrome", "shell-launched Chrome"),
    ("start-process", "shell-launched browser"),
    ("executablepath:", "explicit browser path"),
    ("executablepath=", "explicit browser path"),
    ("channel: 'chrome'", "Playwright channel override"),
    ("channel: \"chrome\"", "Playwright channel override"),
    ("channel: 'msedge'", "Playwright channel override"),
    ("channel: \"msedge\"", "Playwright channel override"),
    ("--headless", "raw browser CLI"),
    ("--screenshot=", "raw browser CLI"),
    ("--user-data-dir", "raw browser CLI"),
];

fn workspace() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../")
}

fn keep(path: &Path) -> bool {
    let name = path.file_name().and_then(|name| name.to_str()).unwrap_or_default();
    // The gate itself names the forbidden patterns, and it is not a browser
    // driver. Everything else in these trees is scanned.
    if name == "browser_toolchain.rs" {
        return false;
    }
    if path.components().any(|part| matches!(part.as_os_str().to_str(), Some("node_modules" | "target"))) {
        return false;
    }
    matches!(path.extension().and_then(|extension| extension.to_str()), Some("rs" | "mjs" | "js" | "ts" | "yml" | "yaml" | "ps1" | "sh"))
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(&path, out);
        }
        else if keep(&path) {
            out.push(path);
        }
    }
}

/// Every `(pattern, line number)` that matches a forbidden browser escape.
fn violations_in(text: &str) -> Vec<(&'static str, usize)> {
    let lowered = text.to_lowercase();
    let mut found = Vec::new();
    for (pattern, _) in FORBIDDEN {
        for (index, _) in lowered.match_indices(pattern) {
            let line = lowered[..index].matches('\n').count() + 1;
            found.push((*pattern, line));
        }
    }
    found
}

#[test]
fn no_test_or_script_launches_a_system_browser() {
    let root = workspace();
    let mut files = Vec::new();
    for relative in ["projects/crates", "scripts", ".github/workflows"] {
        collect(&root.join(relative), &mut files);
    }
    assert!(files.len() > 5, "the browser gate scanned {} files, which looks wrong", files.len());

    let mut offenders = Vec::new();
    for file in &files {
        let text = fs::read_to_string(file).unwrap_or_else(|error| panic!("cannot read {}: {error}", file.display()));
        for (pattern, line) in violations_in(&text) {
            offenders.push(format!("{}:{}: `{pattern}` ({})", file.display(), line, FORBIDDEN.iter().find(|(candidate, _)| *candidate == pattern).map(|(_, why)| *why).unwrap_or("forbidden")));
        }
    }
    assert!(offenders.is_empty(), "{MESSAGE}\n{}", offenders.join("\n"));
}

/// Prove the scanner really goes red on a system-Chrome launch.
#[test]
fn the_scanner_flags_a_system_chrome_launch() {
    let sample = concat!(
        "Command::new(\"C:/Program Files/Google/Chrome/Application/chrome.exe\")\n",
        "    .args([\"--headless\", \"--screenshot=out.png\", \"--user-data-dir=C:/tmp\"]);\n",
        "chromium.launch({ channel: 'chrome', executablePath: 'C:/chrome.exe' });\n",
    );
    let found = violations_in(sample);
    assert!(found.iter().any(|(pattern, line)| *pattern == "chrome.exe" && *line == 1), "the scanner missed `chrome.exe`: {found:?}");
    assert!(found.iter().any(|(pattern, line)| *pattern == "--headless" && *line == 2), "the scanner missed a raw browser CLI: {found:?}");
    assert!(found.iter().any(|(pattern, _)| *pattern == "program files/google"), "the scanner missed the install path: {found:?}");
    assert!(found.iter().any(|(pattern, _)| *pattern == "channel: 'chrome'"), "the scanner missed the `channel` override: {found:?}");
    assert!(found.iter().any(|(pattern, _)| *pattern == "executablepath:"), "the scanner missed `executablePath`: {found:?}");

    // A domain field and a prose mention must stay green.
    let benign = concat!(
        "pub struct Encoding { pub channel: Channel }\n",
        "// `chromium.launch()` with no `channel` and no `executablePath`\n",
    );
    assert!(violations_in(benign).is_empty(), "the scanner must not flag domain fields or prose: {:?}", violations_in(benign));
}
