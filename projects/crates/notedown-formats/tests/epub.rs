use notedown_formats::export::markdown::export_markdown;
use notedown_formats::import::epub::import_epub_bytes;

fn stored_zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut archive = Vec::new();
    let mut central = Vec::new();

    for (path, payload) in entries {
        let name = path.as_bytes();
        let crc = crc32(payload);
        let local_offset = archive.len();

        archive.extend_from_slice(b"PK\x03\x04");
        archive.extend_from_slice(&[0x14, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]);
        archive.extend_from_slice(&crc.to_le_bytes());
        archive.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        archive.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        archive.extend_from_slice(&(name.len() as u16).to_le_bytes());
        archive.extend_from_slice(&[0x00, 0x00]);
        archive.extend_from_slice(name);
        archive.extend_from_slice(payload);

        central.extend_from_slice(b"PK\x01\x02");
        let mut cd_fixed = [0u8; 46];
        cd_fixed[0..2].copy_from_slice(&[0x14, 0x00]);
        cd_fixed[2..4].copy_from_slice(&[0x14, 0x00]);
        cd_fixed[12..16].copy_from_slice(&crc.to_le_bytes());
        cd_fixed[16..20].copy_from_slice(&(payload.len() as u32).to_le_bytes());
        cd_fixed[20..24].copy_from_slice(&(payload.len() as u32).to_le_bytes());
        cd_fixed[24..26].copy_from_slice(&(name.len() as u16).to_le_bytes());
        cd_fixed[38..42].copy_from_slice(&(local_offset as u32).to_le_bytes());
        central.extend_from_slice(&cd_fixed);
        central.extend_from_slice(name);
    }

    let cd_offset = archive.len();
    archive.extend_from_slice(&central);
    let cd_size = central.len();
    archive.extend_from_slice(b"PK\x05\x06");
    archive.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]);
    archive.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    archive.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    archive.extend_from_slice(&(cd_size as u32).to_le_bytes());
    archive.extend_from_slice(&(cd_offset as u32).to_le_bytes());
    archive.extend_from_slice(&[0x00, 0x00]);
    archive
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for byte in data {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            let bit = crc & 1;
            crc >>= 1;
            if bit != 0 {
                crc ^= 0xEDB8_8320;
            }
        }
    }
    crc ^ 0xFFFF_FFFF
}

fn minimal_epub_zip() -> Vec<u8> {
    let container = br#"<?xml version="1.0" encoding="UTF-8"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <rootfiles>
    <rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/>
  </rootfiles>
</container>"#;
    let opf = br#"<?xml version="1.0" encoding="UTF-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="uid">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:title>Sample Book</dc:title>
    <dc:language>en</dc:language>
  </metadata>
  <manifest>
    <item id="ch1" href="chapter.xhtml" media-type="application/xhtml+xml"/>
  </manifest>
  <spine>
    <itemref idref="ch1"/>
  </spine>
</package>"#;
    stored_zip(&[
        ("mimetype", b"application/epub+zip"),
        ("META-INF/container.xml", container),
        ("OEBPS/content.opf", opf),
        (
            "OEBPS/chapter.xhtml",
            br#"<?xml version="1.0" encoding="UTF-8"?>
<html xmlns="http://www.w3.org/1999/xhtml">
  <body>
    <h1>Chapter One</h1>
    <p>Hello EPUB</p>
  </body>
</html>"#,
        ),
    ])
}

#[test]
fn epub_import_maps_metadata_and_spine_xhtml() {
    let zip = minimal_epub_zip();
    let graph = import_epub_bytes("book.epub", &zip).expect("import epub");
    assert_eq!(graph.metadata.title.as_deref(), Some("Sample Book"));
    assert_eq!(graph.metadata.language.as_deref(), Some("en"));
    assert!(graph.blocks.len() >= 2);
    let markdown = export_markdown(&graph).expect("export markdown");
    assert!(markdown.contains("# Chapter One"));
    assert!(markdown.contains("Hello EPUB"));
}

fn epub_with_nav_zip() -> Vec<u8> {
    let container = br#"<?xml version="1.0" encoding="UTF-8"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <rootfiles>
    <rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/>
  </rootfiles>
</container>"#;
    let opf = br#"<?xml version="1.0" encoding="UTF-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="uid">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:title>Nav Book</dc:title>
    <dc:language>en</dc:language>
  </metadata>
  <manifest>
    <item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>
    <item id="ch1" href="chapter.xhtml" media-type="application/xhtml+xml"/>
  </manifest>
  <spine>
    <itemref idref="ch1"/>
  </spine>
</package>"#;
    let nav = br#"<?xml version="1.0" encoding="UTF-8"?>
<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops">
  <body>
    <nav epub:type="toc" id="toc">
      <ol>
        <li><a href="chapter.xhtml">Chapter One</a></li>
      </ol>
    </nav>
  </body>
</html>"#;
    stored_zip(&[
        ("mimetype", b"application/epub+zip"),
        ("META-INF/container.xml", container),
        ("OEBPS/content.opf", opf),
        ("OEBPS/nav.xhtml", nav),
        (
            "OEBPS/chapter.xhtml",
            br#"<?xml version="1.0" encoding="UTF-8"?>
<html xmlns="http://www.w3.org/1999/xhtml">
  <body>
    <h1>Chapter One</h1>
    <p>Body text</p>
  </body>
</html>"#,
        ),
    ])
}

#[test]
fn epub_import_maps_navigation_toc() {
    let zip = epub_with_nav_zip();
    let graph = import_epub_bytes("nav.epub", &zip).expect("import epub");
    let markdown = export_markdown(&graph).expect("export markdown");
    assert!(markdown.contains("[Chapter One](chapter.xhtml)"));
    assert!(markdown.contains("# Chapter One"));
}

fn epub_with_image_zip() -> Vec<u8> {
    let container = br#"<?xml version="1.0" encoding="UTF-8"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <rootfiles>
    <rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/>
  </rootfiles>
</container>"#;
    let opf = br#"<?xml version="1.0" encoding="UTF-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="uid">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:title>Image Book</dc:title>
    <dc:language>en</dc:language>
  </metadata>
  <manifest>
    <item id="cover" href="images/cover.png" media-type="image/png" properties="cover-image"/>
    <item id="ch1" href="chapter.xhtml" media-type="application/xhtml+xml"/>
  </manifest>
  <spine>
    <itemref idref="ch1"/>
  </spine>
</package>"#;
    stored_zip(&[
        ("mimetype", b"application/epub+zip"),
        ("META-INF/container.xml", container),
        ("OEBPS/content.opf", opf),
        ("OEBPS/images/cover.png", b"\x89PNG\r\n"),
        (
            "OEBPS/chapter.xhtml",
            br#"<?xml version="1.0" encoding="UTF-8"?>
<html xmlns="http://www.w3.org/1999/xhtml">
  <body>
    <p>Before image</p>
    <img src="images/cover.png" alt="Cover art"/>
  </body>
</html>"#,
        ),
    ])
}

#[test]
fn epub_import_registers_embedded_images() {
    let zip = epub_with_image_zip();
    let graph = import_epub_bytes("image.epub", &zip).expect("import epub");
    assert!(graph.assets.len() >= 1);
    assert!(graph.assets.iter().any(|asset| asset.bytes.is_some()));
    let markdown = export_markdown(&graph).expect("export markdown");
    assert!(markdown.contains("![Cover art](images/cover.png)"));
    assert!(markdown.contains("Before image"));
}

fn epub_with_stylesheet_and_svg_zip() -> Vec<u8> {
    let container = br#"<?xml version="1.0" encoding="UTF-8"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <rootfiles>
    <rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/>
  </rootfiles>
</container>"#;
    let opf = br#"<?xml version="1.0" encoding="UTF-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="uid">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:title>Styled Book</dc:title>
    <dc:language>en</dc:language>
  </metadata>
  <manifest>
    <item id="css" href="styles/main.css" media-type="text/css"/>
    <item id="ch1" href="chapter.xhtml" media-type="application/xhtml+xml"/>
    <item id="logo" href="images/logo.png" media-type="image/png"/>
  </manifest>
  <spine>
    <itemref idref="ch1"/>
  </spine>
</package>"#;
    stored_zip(&[
        ("mimetype", b"application/epub+zip"),
        ("META-INF/container.xml", container),
        ("OEBPS/content.opf", opf),
        ("OEBPS/styles/main.css", b"body { color: red; }"),
        ("OEBPS/images/logo.png", b"\x89PNG\r\n"),
        (
            "OEBPS/chapter.xhtml",
            br#"<?xml version="1.0" encoding="UTF-8"?>
<html xmlns="http://www.w3.org/1999/xhtml"
      xmlns:xlink="http://www.w3.org/1999/xlink">
  <head>
    <link rel="stylesheet" href="styles/main.css"/>
  </head>
  <body>
    <p style="color: blue;">Styled text</p>
    <svg>
      <image xlink:href="images/logo.png" alt="Logo"/>
    </svg>
  </body>
</html>"#,
        ),
    ])
}

#[test]
fn epub_import_registers_stylesheet_assets() {
    let zip = epub_with_stylesheet_and_svg_zip();
    let graph = import_epub_bytes("styled.epub", &zip).expect("import epub");
    assert!(
        graph
            .assets
            .iter()
            .any(|asset| asset.media_type.as_deref() == Some("text/css"))
    );
    assert!(
        graph
            .coverage
            .loss
            .iter()
            .any(|loss| loss.code == "import.epub.css_link_unsupported")
    );
    assert!(
        graph
            .coverage
            .loss
            .iter()
            .any(|loss| loss.code == "import.epub.inline_style_unsupported")
    );
}

#[test]
fn epub_import_lowers_svg_external_image_references() {
    let zip = epub_with_stylesheet_and_svg_zip();
    let graph = import_epub_bytes("svg.epub", &zip).expect("import epub");
    let markdown = export_markdown(&graph).expect("export markdown");
    assert!(markdown.contains("![Logo](images/logo.png)"));
}

fn epub_with_table_and_nested_list_zip() -> Vec<u8> {
    let container = br#"<?xml version="1.0" encoding="UTF-8"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <rootfiles>
    <rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/>
  </rootfiles>
</container>"#;
    let opf = br#"<?xml version="1.0" encoding="UTF-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="uid">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:title>Structure Book</dc:title>
    <dc:language>en</dc:language>
  </metadata>
  <manifest>
    <item id="ch1" href="chapter.xhtml" media-type="application/xhtml+xml"/>
  </manifest>
  <spine>
    <itemref idref="ch1"/>
  </spine>
</package>"#;
    stored_zip(&[
        ("mimetype", b"application/epub+zip"),
        ("META-INF/container.xml", container),
        ("OEBPS/content.opf", opf),
        (
            "OEBPS/chapter.xhtml",
            br#"<?xml version="1.0" encoding="UTF-8"?>
<html xmlns="http://www.w3.org/1999/xhtml">
  <body>
    <table>
      <tr><th>Name</th><th>Value</th></tr>
      <tr><td>Alpha</td><td>1</td></tr>
    </table>
    <ul>
      <li>Outer
        <ul>
          <li>Inner</li>
        </ul>
      </li>
    </ul>
  </body>
</html>"#,
        ),
    ])
}

fn epub_with_block_elements_zip() -> Vec<u8> {
    let container = br#"<?xml version="1.0" encoding="UTF-8"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <rootfiles>
    <rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/>
  </rootfiles>
</container>"#;
    let opf = br#"<?xml version="1.0" encoding="UTF-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="uid">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:title>Blocks Book</dc:title>
    <dc:language>en</dc:language>
  </metadata>
  <manifest>
    <item id="ch1" href="chapter.xhtml" media-type="application/xhtml+xml"/>
  </manifest>
  <spine>
    <itemref idref="ch1"/>
  </spine>
</package>"#;
    stored_zip(&[
        ("mimetype", b"application/epub+zip"),
        ("META-INF/container.xml", container),
        ("OEBPS/content.opf", opf),
        (
            "OEBPS/chapter.xhtml",
            br#"<?xml version="1.0" encoding="UTF-8"?>
<html xmlns="http://www.w3.org/1999/xhtml">
  <body>
    <section>
      <p>Inside section</p>
    </section>
    <blockquote><p>Quoted line</p></blockquote>
    <pre><code class="language-rust">fn main() {}</code></pre>
    <hr/>
  </body>
</html>"#,
        ),
    ])
}

#[test]
fn epub_import_lowers_container_blockquote_code_and_hr() {
    let zip = epub_with_block_elements_zip();
    let graph = import_epub_bytes("blocks.epub", &zip).expect("import epub");
    let markdown = export_markdown(&graph).expect("export markdown");
    assert!(markdown.contains("Inside section"));
    assert!(markdown.contains("> Quoted line"));
    assert!(markdown.contains("```rust"));
    assert!(markdown.contains("fn main() {}"));
    assert!(markdown.contains("---"));
}

fn epub_with_figure_zip() -> Vec<u8> {
    let container = br#"<?xml version="1.0" encoding="UTF-8"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <rootfiles>
    <rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/>
  </rootfiles>
</container>"#;
    let opf = br#"<?xml version="1.0" encoding="UTF-8"?>
<package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="uid">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:title>Figure Book</dc:title>
    <dc:language>en</dc:language>
  </metadata>
  <manifest>
    <item id="photo" href="images/photo.png" media-type="image/png"/>
    <item id="ch1" href="chapter.xhtml" media-type="application/xhtml+xml"/>
  </manifest>
  <spine>
    <itemref idref="ch1"/>
  </spine>
</package>"#;
    stored_zip(&[
        ("mimetype", b"application/epub+zip"),
        ("META-INF/container.xml", container),
        ("OEBPS/content.opf", opf),
        ("OEBPS/images/photo.png", b"\x89PNG\r\n"),
        (
            "OEBPS/chapter.xhtml",
            br#"<?xml version="1.0" encoding="UTF-8"?>
<html xmlns="http://www.w3.org/1999/xhtml">
  <body>
    <figure>
      <img src="images/photo.png" alt="Sunset"/>
      <figcaption>Sunset over the bay</figcaption>
    </figure>
  </body>
</html>"#,
        ),
    ])
}

#[test]
fn epub_import_lowers_figure_image_and_figcaption() {
    let zip = epub_with_figure_zip();
    let graph = import_epub_bytes("figure.epub", &zip).expect("import epub");
    let markdown = export_markdown(&graph).expect("export markdown");
    assert!(markdown.contains("![Sunset](images/photo.png)"));
    assert!(markdown.contains("*Sunset over the bay*"));
}

#[test]
fn epub_import_lowers_tables_and_nested_lists() {
    let zip = epub_with_table_and_nested_list_zip();
    let graph = import_epub_bytes("structure.epub", &zip).expect("import epub");
    let markdown = export_markdown(&graph).expect("export markdown");
    assert!(markdown.contains("| Name | Value |"));
    assert!(markdown.contains("| Alpha | 1 |"));
    assert!(markdown.contains("- Outer"));
    assert!(markdown.contains("  - Inner"));
}
