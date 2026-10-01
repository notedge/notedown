/// Collects hyperlink and image targets for `word/_rels/document.xml.rels`.
#[derive(Debug, Default)]
pub struct DocumentRelsRegistry {
    entries: Vec<RelEntry>,
}

#[derive(Debug, Clone)]
enum RelEntry {
    Hyperlink { url: String },
    Image { target: String, bytes: Vec<u8> },
}

impl DocumentRelsRegistry {
    /// Registers an external hyperlink target and returns a stable relationship id.
    pub fn id_for_hyperlink(&mut self, url: &str) -> String {
        if let Some((index, _)) = self
            .entries
            .iter()
            .enumerate()
            .find(|(_, entry)| matches!(entry, RelEntry::Hyperlink { url: existing } if existing == url))
        {
            return format!("rId{}", index + 1);
        }
        self.entries.push(RelEntry::Hyperlink {
            url: url.to_string(),
        });
        format!("rId{}", self.entries.len())
    }

    /// Registers an embedded image part and returns a stable relationship id.
    pub fn id_for_image(&mut self, target: &str, bytes: Vec<u8>) -> String {
        if let Some((index, _)) = self
            .entries
            .iter()
            .enumerate()
            .find(|(_, entry)| matches!(entry, RelEntry::Image { target: existing, .. } if existing == target))
        {
            return format!("rId{}", index + 1);
        }
        self.entries.push(RelEntry::Image {
            target: target.to_string(),
            bytes,
        });
        format!("rId{}", self.entries.len())
    }

    /// Whether any relationships were collected.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// ZIP members for embedded images keyed by OPC part path (e.g. `word/media/logo.png`).
    pub fn media_parts(&self) -> Vec<(String, Vec<u8>)> {
        self.entries
            .iter()
            .filter_map(|entry| match entry {
                RelEntry::Image { target, bytes } => Some((format!("word/{target}"), bytes.clone())),
                RelEntry::Hyperlink { .. } => None,
            })
            .collect()
    }

    /// File extensions for embedded images (e.g. `png`) for `[Content_Types].xml`.
    pub fn image_extensions(&self) -> Vec<String> {
        let mut extensions = Vec::new();
        for entry in &self.entries {
            if let RelEntry::Image { target, .. } = entry {
                if let Some(extension) = target.rsplit('.').next() {
                    let extension = extension.to_ascii_lowercase();
                    if !extensions.contains(&extension) {
                        extensions.push(extension);
                    }
                }
            }
        }
        extensions
    }

    /// Renders `word/_rels/document.xml.rels` for collected relationships.
    pub fn render_document_rels_xml(&self) -> String {
        if self.entries.is_empty() {
            return r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
</Relationships>"#
                .to_string();
        }

        let mut xml = String::from(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">"#,
        );
        for (index, entry) in self.entries.iter().enumerate() {
            xml.push_str("\n  <Relationship Id=\"rId");
            xml.push_str(&(index + 1).to_string());
            xml.push_str("\" ");
            match entry {
                RelEntry::Hyperlink { url } => {
                    xml.push_str(
                        "Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink\" Target=\"",
                    );
                    xml.push_str(&escape_xml_attr(url));
                    xml.push_str("\" TargetMode=\"External\"/>");
                }
                RelEntry::Image { target, .. } => {
                    xml.push_str(
                        "Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/image\" Target=\"",
                    );
                    xml.push_str(&escape_xml_attr(target));
                    xml.push_str("\"/>");
                }
            }
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
