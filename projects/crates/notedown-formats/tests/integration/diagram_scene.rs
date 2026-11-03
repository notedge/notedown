//! End-to-end acceptance for the diagram chain the Pandark crawl pipeline
//! relies on: real-world HTML mermaid containers become `Block::Diagram`
//! without losing the authored source, and the DaVinci diagram subsystem
//! lowers that source into a renderable SVG scene.

use std::{fs, path::PathBuf};

use davinci_diagram::mermaid_to_scene;
use notedown_formats::import::html::import_html_bytes;
use notedown_ir::Block;

/// A docs page fragment shaped the way real sites embed diagrams: the Mermaid
/// runtime convention (`<div class="mermaid">`) and the static-site convention
/// (`<pre><code class="language-mermaid">`).
const PAGE: &str = r#"<!DOCTYPE html>
<html><head><title>Crawl pipeline notes</title></head><body>
<article>
<h1>Crawl pipeline</h1>
<p>The fetch to extract flow:</p>
<div class="mermaid">
flowchart TD
  A[Fetch page] --> B{Status OK?}
  B -->|yes| C[Extract blocks]
  B -->|no| D[Record failure]
</div>
<p>And the worker handshake:</p>
<pre><code class="language-mermaid">sequenceDiagram
  Frontier-&gt;&gt;Worker: claim
  Worker-&gt;&gt;Frontier: commit</code></pre>
</article>
</body></html>"#;

fn diagram_sources(label: &str) -> Vec<String> {
    let graph = import_html_bytes(label, PAGE.as_bytes()).expect("import html page");
    let sources = graph
        .blocks
        .iter()
        .filter_map(|node| match &node.block {
            Block::Diagram { language, source } => {
                assert_eq!(language, "mermaid", "{label}: diagram language");
                Some(source.clone())
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(sources.len(), 2, "{label}: both mermaid containers must become diagrams: {:?}", graph.blocks);
    sources
}

#[test]
fn html_mermaid_containers_lower_to_renderable_svg_scenes() {
    let sources = diagram_sources("pipeline.html");
    assert!(sources[0].contains("flowchart TD"), "div container keeps the flowchart source:\n{}", sources[0]);
    assert!(sources[1].contains("sequenceDiagram"), "fenced code keeps the sequence source:\n{}", sources[1]);

    let out_dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("diagram-artifacts");
    fs::create_dir_all(&out_dir).expect("create diagram artifact dir");

    let names = ["flowchart", "sequence"];
    let expected_labels: [&[&str]; 2] = [
        &["Fetch page", "Status OK?", "Extract blocks", "Record failure", "yes"],
        &["Frontier", "Worker", "claim", "commit"],
    ];
    for ((name, source), labels) in names.iter().zip(sources.iter()).zip(expected_labels.iter()) {
        let lowered = mermaid_to_scene(source).unwrap_or_else(|error| panic!("{name}: mermaid lowering failed: {error}\n{source}"));
        let svg = lowered.scene.to_svg().unwrap_or_else(|error| panic!("{name}: scene must serialize to SVG: {error}"));
        for label in *labels {
            assert!(svg_contains_label(&svg, label), "{name}: SVG must draw the label `{label}`\n{svg}");
        }
        let path = out_dir.join(format!("{name}.svg"));
        fs::write(&path, &svg).expect("write diagram svg artifact");
        eprintln!("wrote {}", path.display());
    }
}

fn svg_contains_label(svg: &str, label: &str) -> bool {
    svg.contains(&format!(">{label}</text>")) || svg.contains(&format!(">{label} ")) || svg.contains(label)
}
