/// Collects external hyperlink targets for `word/_rels/document.xml.rels`.
#[derive(Debug, Default)]
pub struct HyperlinkRegistry {
    targets: Vec<String>,
}

impl HyperlinkRegistry {
    /// Registers a hyperlink target and returns a stable relationship id.
    pub fn id_for(&mut self, url: &str) -> String {
        if let Some((index, _)) = self
            .targets
            .iter()
            .enumerate()
            .find(|(_, target)| *target == url)
        {
            return format!("rId{}", index + 1);
        }
        self.targets.push(url.to_string());
        format!("rId{}", self.targets.len())
    }

    /// Whether any hyperlink relationships were collected.
    pub fn is_empty(&self) -> bool {
        self.targets.is_empty()
    }

    /// Renders `word/_rels/document.xml.rels` for collected hyperlinks.
    pub fn render_document_rels_xml(&self) -> String {
        if self.targets.is_empty() {
            return r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
</Relationships>"#
                .to_string();
        }

        let mut xml = String::from(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">"#,
        );
        for (index, target) in self.targets.iter().enumerate() {
            xml.push_str("\n  <Relationship Id=\"rId");
            xml.push_str(&(index + 1).to_string());
            xml.push_str(
                "\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink\" Target=\"",
            );
            xml.push_str(&escape_xml_attr(target));
            xml.push_str("\" TargetMode=\"External\"/>");
        }
        xml.push_str("\n</Relationships>");
        xml
    }
}

fn escape_xml_attr(value: &str) -> String {
    let mut output = String::new();
    for ch in value.chars() {
        match ch {
            '&' => output.push_str("&amp;"),
            '<' => output.push_str("&lt;"),
            '"' => output.push_str("&quot;"),
            _ => output.push(ch),
        }
    }
    output
}
