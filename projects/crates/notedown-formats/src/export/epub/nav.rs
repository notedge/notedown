//! EPUB 3 `OEBPS/nav.xhtml` generation.

use notedown_ir::DocumentGraph;

use super::chapter::ChapterPlan;
use crate::FormatError;

/// Render the EPUB3 navigation document as XHTML.
pub fn nav_xhtml(graph: &DocumentGraph, chapters: &[ChapterPlan]) -> Result<String, FormatError> {
    let mut out = String::new();
    out.push_str(
        r##"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE html>
<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops" xml:lang="#" lang="#">
  <head>
    <meta charset="UTF-8"/>
    <title>"##,
    );
    if let Some(title) = &graph.metadata.title {
        out.push_str(&escape_xml(title));
    } else {
        out.push_str("Table of Contents");
    }
    out.push_str(
        r#"</title>
  </head>
  <body>
    <nav epub:type="toc" id="toc">
      <h1>"#,
    );
    if let Some(title) = &graph.metadata.title {
        out.push_str(&escape_xml(title));
    } else {
        out.push_str("Table of Contents");
    }
    out.push_str(
        r#"</h1>
      <ol>
"#,
    );
    for (index, _) in chapters.iter().enumerate() {
        let href = super::chapter::chapter_href(index);
        out.push_str("        <li><a href=\"");
        out.push_str(&escape_xml(&href));
        out.push_str("\">");
        out.push_str(&escape_xml(&chapters[index].label));
        out.push_str("</a></li>\n");
    }
    out.push_str(
        r#"      </ol>
    </nav>
  </body>
</html>
"#,
    );
    Ok(out)
}

fn escape_xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
