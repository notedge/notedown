use std::collections::HashMap;

use notedown_formats::{
    export::markdown::export_markdown,
    import::docx::{
        footnotes::FootnoteCatalog,
        numbering::NumberingCatalog,
        xml::{new_graph, parse_document_xml},
    },
};
use notedown_ir::SemanticStatus;

#[test]
fn parses_footnote_reference_inside_run() {
    let xml = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p>
      <w:r><w:t>See</w:t></w:r>
      <w:r><w:footnoteReference w:id="1"/></w:r>
      <w:r><w:t> for details.</w:t></w:r>
    </w:p>
  </w:body>
</w:document>"#;
    let mut graph = new_graph("footnote");
    let referenced =
        parse_document_xml(xml, &HashMap::new(), &NumberingCatalog::default(), &FootnoteCatalog::new(), &mut graph).expect("parse document");
    assert!(referenced.contains(&1));
    let markdown = export_markdown(&graph).expect("export markdown");
    assert!(markdown.contains("See[^1] for details."), "markdown was: {markdown:?}, blocks: {:?}", graph.blocks);
    assert!(graph.coverage.loss.iter().any(|loss| { loss.code == "reader.docx.footnote_body" && loss.status == SemanticStatus::Unresolved }));
}

#[test]
fn parses_bold_and_italic_runs() {
    let xml = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p>
      <w:r><w:rPr><w:b/></w:rPr><w:t>Bold</w:t></w:r>
      <w:r><w:rPr><w:i/></w:rPr><w:t> italic</w:t></w:r>
    </w:p>
  </w:body>
</w:document>"#;
    let mut graph = new_graph("styled");
    parse_document_xml(xml, &HashMap::new(), &NumberingCatalog::default(), &FootnoteCatalog::new(), &mut graph).expect("parse document");
    let markdown = export_markdown(&graph).expect("export markdown");
    assert!(markdown.contains("**Bold**"));
    assert!(markdown.contains("* italic*"), "markdown was: {markdown}");
}
